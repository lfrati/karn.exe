//! Oracle syntax lowering and lexical bindings. Runtime frames never enter this module.
//! Definitions and runtime compilation share these builders and their refusal boundaries.
use super::*;

mod rounding;
mod value_iteration;

/// Close the optional or conditional block that began with the branch at `at` and ends
/// here: say where the branch skips to and what the skipped path leaves in the slots that
/// later text may read, and hand the names of those results from the block's scope (`inner`)
/// to the scope around it (`outer`). Three things differ and are kept apart: a group the
/// block would have made is empty ([`Instr::skipped_result`]); a payment the block would
/// have asked for was not made and could not be (false, false); anything else the block
/// bound is not known outside it.
/// Inside an "otherwise" or "instead", what the branch before it did is nothing: "that many"
/// and "the damage dealt this way" of it would read its skipped, empty result ("target player
/// mills X cards. If …, that player mills twice that many cards instead": not twice none). After
/// both, a later sentence still reads whichever was done.
fn alternative_path(binder: &mut Binder, (produced, damage): (usize, usize)) {
    binder.produced.truncate(produced);
    binder.damage.truncate(damage);
}

fn close_branch(instrs: &mut [Instr], at: usize, inner: &Binder, outer: &mut Binder) {
    // A conditional body is not proof of one immediately preceding movement.
    outer.past_inputs.clear();
    outer.completed_subject = outer.completed_subject.joined(inner.completed_subject);
    outer.alternative_result |= inner.alternative_result;
    outer.unrepresented_movement |= inner.unrepresented_movement;
    let end = instrs.len();
    let produced: Vec<_> = instrs[at + 1..end].iter().flat_map(Instr::skipped_result).collect();
    let Instr::Branch { end: target, skipped_values, .. } = &mut instrs[at] else { unreachable!("a block begins with its branch") };
    *target = end;
    let produced_here = |slot| produced.iter().any(|(out, _)| *out == slot);
    if inner.result.as_ref().is_some_and(|(slot, ..)| produced_here(*slot)) {
        outer.result = inner.result.clone();
        outer.branch_result = inner.result.as_ref().map(|(slot, ..)| *slot);
    }
    outer.created_groups.extend(inner.created_groups.iter().filter(|slot| produced_here(**slot)).copied());
    if inner.created_single.is_some_and(produced_here) { outer.created_single = inner.created_single; }
    // A selection from an existing group remains excluded from "the rest" after
    // leaving its branch. On the skipped path that selection is the empty group.
    for group in &mut outer.groups {
        if let Some(selected) = inner.groups.iter().find(|g| g.slot == group.slot) {
            group.taken.extend(selected.taken.iter().copied().filter(|slot| produced_here(*slot)));
        }
    }
    if inner.copies.is_some_and(produced_here) { outer.copies = inner.copies; }
    if inner.created_turn.is_some_and(produced_here) { outer.created_turn = inner.created_turn; }
    outer.groups.extend(inner.groups.iter().filter(|group| produced_here(group.slot)).cloned());
    outer.discarded.extend(inner.discarded.iter().filter(|(_, before)| produced_here(*before)).copied());
    outer.entered.extend(inner.entered.iter().filter(|slot| produced_here(**slot)).copied());
    // what an instruction that was skipped did is nothing, and "this way" still names it
    outer.produced.extend(inner.produced.iter().filter(|p| produced_here(p.slot)).copied());
    outer.damage.extend(inner.damage.iter().filter(|slot| produced_here(**slot)).copied());
    outer.countered.extend(inner.countered.iter().filter(|(_, slot)| produced_here(*slot)).cloned());
    outer.exiled_now.extend(inner.exiled_now.iter().filter(|slot| produced_here(**slot)).copied());
    skipped_values.extend(produced);
    // A skipped conditional payment did not begin. Export its explicit false
    // outcome on this path; the taken path produces the same typed values.
    if inner.outcome != outer.outcome {
        if let Some((_, paid, possible, _)) = &inner.outcome {
            skipped_values.push((*paid, Value::Consent(false)));
            // Mana payments expose only their paid outcome. Their binder aliases
            // `possible` to it; a skipped branch must define that slot just once.
            if possible != paid { skipped_values.push((*possible, Value::Consent(false))); }
        }
        outer.outcome = inner.outcome.clone();
    }
}

/// A group in lexical scope, with what has been taken from it.
#[derive(Clone)]
struct Group {
    slot: Slot,
    /// The paragraph that produced it, of a spell's paragraphs followed as one text.
    paragraph: usize,
    /// The verb of the action that produced it.
    by: Verb,
    zone: Zone,
    taken: Vec<Slot>,
}

/// A completed instruction's result group, as "this way" names it (608.2c): the verb that
/// did it, how its slot holds what it did, whether the controller alone did it, and the zone
/// it put what it did into, when one.
#[derive(Clone, Copy)]
struct Produced { slot: Slot, verb: Verb, shape: expr::Shape, yours: bool, into: Option<Zone> }

/// Something the text named, kept in a slot for what it says of it afterwards.
#[derive(Clone)]
struct Named {
    slot: Slot,
    /// Completed public-zone successor for object actions (400.7j). Characteristic
    /// expressions keep the original card unless their own clause names the successor.
    object: Option<Slot>,
    set: ObjectSet,
    bounds: (u32, u32),
    /// An announced target, rather than an action result or occurrence participant.
    target: bool,
    /// Explicitly selected by a "choose" object instruction.
    explicit_choice: bool,
    /// This captured player is the controller of this particular target slot, or the owner of
    /// the zone a target card was said to be in ("from an opponent's graveyard").
    controlled_target: Option<Slot>,
    /// A direct attached-object mention, superseded by its next direct mention.
    attached: bool,
    /// Named as the ability's own ("it", of "whenever this creature attacks"): the text says
    /// "that creature" of something else where it can.
    own: bool,
    /// The sentence that named it, and the act of it.
    at: (usize, usize),
}

/// What a reference names.
enum Referent {
    /// One slot; several where a plural names everything one act named ("exile target
    /// artifact and target creature. Then return them").
    Local(Vec<Slot>),
    Input(input::Input),
    Source,
}

/// Only the last completed clause can establish the new source-relative proof.
/// A divergent lexical join keeps its unresolved choice until a later clause replaces it.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum CompletedSubject { #[default] Other, Source, Unresolved }
impl CompletedSubject {
    fn joined(self, other: Self) -> Self {
        if self == other { self } else { Self::Unresolved }
    }
}

/// A named dealer affects only its recipient selectors, independently of the
/// effect controller and characteristic-amount possessors (608.2h).
#[derive(Default, Clone)]
struct RecipientContext {
    subject: Option<expr::Subject>,
    distributive: bool,
}

/// Resolves references to producers by lexical scope.
#[derive(Default, Clone)]
struct Binder {
    /// What one act named together as its objects ("untap target creature and each other
    /// creature that shares a color with it"), each list of their slots.
    coordinated: Vec<Vec<Slot>>,
    /// The result an act inside a conditional branch made, carried out of it when it closed.
    branch_result: Option<Slot>,
    /// The exact preceding local flip; unrelated numbers and actions do not replace it.
    coin: Option<Slot>,
    turn: Option<Slot>,
    created_turn: Option<Slot>,
    /// What the text's copy instruction made: "the copy", "the copies" (707.12).
    copies: Option<Slot>,
    groups: Vec<Group>,
    scalars: Vec<(Slot, Kind)>,
    subjects: Vec<Named>,
    /// Original lexical names and the actual inputs of the last movement. These slots
    /// retain the pre-move incarnation, independently of its public-zone successor.
    past_inputs: Vec<(Slot, Slot)>,
    /// Where the text last said "this creature" of its own source: what "it" is in a later
    /// act, where nothing else can be meant. Not in the same act: "attach this Equipment
    /// to it" is not said of the Equipment, which would be "itself".
    source_named: Option<(usize, usize)>,
    /// The sentence being lowered, and the act of it.
    at: (usize, usize),
    /// The sentences of the texts lowered before this one in the same scope.
    sentences: usize,
    /// Where the paragraphs after the first begin among those sentences, of a spell's
    /// paragraphs followed as one text (608.2c).
    paragraphs: Vec<usize>,
    outcome: Option<(action::Operand, Slot, Slot, usize)>,
    player_outcomes: Vec<[Slot; 3]>,
    /// An instruction lowered before has players lose life: "the life lost this way".
    lost: Vec<Slot>,
    damage: Vec<Slot>,
    /// A counter result belongs to its exact named input, never the latest counter in
    /// the frame. Skipped instructions export an empty result like other producers.
    countered: Vec<(action::Operand, Slot)>,
    /// A positive completed-damage condition supplies "that much" only in its branch.
    condition_amount: Option<expr::Amount>,
    /// Explicit opposing participants declared by earlier Fight instructions.
    fight_recipients: Vec<Slot>,
    destroyed: Vec<Slot>,
    removed: Vec<Slot>,
    discarded: Vec<(Slot, Slot)>, // actual result slot and its explicit original-card projection
    /// Identities before and after completed battlefield-entry instructions (608.2c, 400.7).
    entered: Vec<Slot>,
    exiled: Vec<Slot>,
    /// Every completed instruction's result group in this scope, by the verb that did it,
    /// in order: what "this way" names (608.2c).
    produced: Vec<Produced>,
    /// A movement whose complete two-destination result is not represented here.
    /// Generic tallies must not fall back to an older or partial producer (608.2c).
    unrepresented_movement: bool,
    /// What earlier instructions of this text put into exile, as it is there: what "the
    /// exiled cards" names before any card exiled by another ability (608.2c, 400.7).
    exiled_now: Vec<Slot>,
    /// The explicit card-producing action in this discourse scope. Search opens a new
    /// result scope; empty output still binds it. This is lexical, never a log lookup.
    result: Option<(Slot, ObjectSet, u32)>,
    /// What a result no subject names became as this text put it onto the battlefield: "put
    /// it onto the battlefield tapped, … untap that land" untaps the permanent (400.7j).
    result_entered: Option<(Slot, Slot)>,
    /// Result-free alternatives cannot supply an object anaphor or reuse an older producer.
    alternative_result: bool,
    /// A printed single token may become a group under 614.6. Its singular noun
    /// still names that complete group; it supplies no runtime cardinality proof.
    created_single: Option<Slot>,
    created_groups: Vec<Slot>,
    searched: Option<Slot>,
    /// A fixed searched group has a common source zone only for a single-zone search.
    searched_zone: Option<Zone>,
    found_waiting: bool,
    /// The players of the "for each player" the text is under.
    each: Option<Slot>,
    /// Each of those players does the acts said under it, rather than the controller for them.
    each_does: bool,
    /// The player who does the clause being lowered, when it is another than the controller
    /// ("Its controller may search their library …"): named once, in this slot.
    actor: Option<Slot>,
    /// The piles the text had cards separated into: the one chosen, the other, and the
    /// zone the cards are in.
    piles: Option<(Slot, Slot, Zone)>,
    /// The subject of the clause being lowered, once it is bound to the source or to a
    /// slot: whose the clause's own "its" and "their" are.
    clause: Option<expr::Subject>,
    completed_subject: CompletedSubject,
    /// Amount possessors of a distributive damage clause name its current source (608.2h).
    object_each: bool,
    /// Temporary actor and distribution proof while binding recipient selectors.
    recipients: RecipientContext,
    /// An adjacent quantified clause contrasted with the stated following root set.
    contrast: Option<(Slot, ObjectSet)>,
    /// The clause being lowered says what X is ("where X is …") and it is not computed yet:
    /// whether the act being lowered reads it.
    x: Option<std::cell::Cell<bool>>,
}

/// `to_players`: inside a damage action whose recipients may be players, where a per-source
/// object quantity is not evaluated.
struct Scoped<'a, 's> { binder: &'a Binder, support: &'a Support<'s>, actor: bool, object_actor: bool, to_players: bool }
impl expr::Binding for Scoped<'_, '_> {
    fn cast_payment_available(&self) -> bool { true }
    fn singular_read(&self, subject: &expr::Subject) -> Res<()> {
        if matches!(subject, expr::Subject::Local(slot) if self.binder.created_groups.contains(slot)) {
            return unsupported("singular scalar or condition of a replacement-expanded created group");
        }
        Ok(())
    }
    fn other_subject(&self) -> Option<expr::Subject> { self.binder.recipients.subject.clone() }
    fn root_other(&self, set: &ObjectSet) -> Res<Option<expr::Subject>> { self.binder.root_other(set, self.support) }
    fn root_controlled_other(&self, set: &ObjectSet, player: &Selector) -> Res<Option<expr::Subject>> {
        // "You control another Knight" on a permanent is source-relative, even
        // beside an any-controller target. Only a proven target-relative noun relation
        // can override it; possibility overlap alone does not establish that reference.
        if crate::select::is_you(player) { return self.binder.other_target(set, self.support); }
        let mut root = set.clone();
        // The Controls evaluator already restricts the group to this player. Carry that
        // relation only into lexical proof; do not add a second runtime player predicate.
        if root.controller.is_none() {
            root.controller = Some(ir::Relation { who: Box::new(player.clone()), negated: false, past: false });
        }
        self.binder.root_other(&root, self.support)
    }
    fn scalar(&mut self, r: &UnresolvedRef) -> Res<Scalar> { self.binder.scalar(r, self.support) }
    fn subject(&mut self, s: &Selector) -> Res<expr::Subject> {
        Ok(match self.amount_subject(s)? {
            expr::Subject::Local(slot) => expr::Subject::Local(self.binder.object(slot)),
            other => other,
        })
    }
    fn comparison_subject(&mut self, s: &Selector) -> Res<expr::Subject> {
        self.binder.check_alternative_reference(s)?;
        // An explicit exile result or linked producer is already proven independently
        // of clause possession; preserve that binding before ordinary anaphor lookup.
        if matches!(s.quant, Quant::Definite(Ref::Anaphor(ir::Anaphor::TheParticiple { what: Verb::Exile, .. }))) {
            return self.subject(s);
        }
        match self.binder.referent(s, self.support, false)? {
            Referent::Local(slots) => match slots.as_slice() {
                [slot] => Ok(expr::Subject::Local(self.binder.object(*slot))),
                _ => unsupported("reference: several named things where one is read"),
            },
            Referent::Input(input) => Ok(expr::Subject::Input(input)),
            Referent::Source => Ok(expr::Subject::Source),
        }
    }
    fn amount_subject(&mut self, s: &Selector) -> Res<expr::Subject> {
        self.binder.check_alternative_reference(s)?;
        // The singular member of the player group whose quantity is being evaluated.
        if self.actor && self.binder.clause == Some(expr::Subject::Actor)
            && s.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::That))
            && s.set == ObjectSet::of(Domain::Player) && s.chooser.is_none() && s.random.is_none()
        { return Ok(expr::Subject::Actor); }
        // "the exiled card": what this text exiled, as it is in exile; else what an ability
        // linked to this one exiled (607.2a)
        if let Quant::Definite(Ref::Anaphor(ir::Anaphor::TheParticiple { what: Verb::Exile, .. })) = s.quant {
            return match self.binder.exiled_now.as_slice() {
                [] => Ok(expr::Subject::LinkedExile(self.support.exile_producer()?)),
                [one] => Ok(expr::Subject::Local(*one)),
                _ => unsupported("the exiled card: of several exiles"),
            };
        }
        // every subject an expression asks for is a possessor: "its power", "their hand"
        if let Some(owner) = self.binder.possessor(s) {
            if matches!(owner, expr::Subject::Actor) && !self.actor { return unsupported("per-player quantity outside a player action"); }
            if matches!(owner, expr::Subject::ObjectActor) && self.to_players { return unsupported("per-source object quantity in damage a player may be dealt"); }
            if matches!(owner, expr::Subject::ObjectActor) && !self.object_actor { return unsupported("per-source object quantity outside a damage action"); }
            return Ok(owner);
        }
        match self.binder.referent(s, self.support, false)? {
            Referent::Local(slots) => match slots.as_slice() {
                [slot] => Ok(expr::Subject::Local(*slot)),
                _ => unsupported("reference: several named things where one is read"),
            },
            Referent::Input(input) => Ok(expr::Subject::Input(input)),
            Referent::Source => Ok(expr::Subject::Source),
        }
    }
    fn past_subject(&mut self, s: &Selector) -> Res<expr::Subject> {
        let expr::Subject::Local(named) = self.amount_subject(s)? else {
            return unsupported("past condition: no retained movement input");
        };
        self.singular_read(&expr::Subject::Local(named))?;
        self.binder.past_inputs.iter().find(|(name, _)| *name == named)
            .map(|(_, input)| expr::Subject::Local(*input))
            .ok_or_else(|| crate::Unsupported("past condition: no retained movement input".into()))
    }
    fn x(&mut self) -> bool {
        self.binder.x.as_ref().is_some_and(|read| { read.set(true); true })
    }
    fn removals(&mut self) -> Res<Slot> {
        match self.binder.removed.as_slice() { [slot] => Ok(*slot), _ => unsupported("counter removal result: absent or ambiguous producer") }
    }
    fn destructions(&mut self) -> Res<Slot> {
        match self.binder.destroyed.as_slice() {
            [slot] => Ok(*slot),
            _ => unsupported("destruction result: absent or ambiguous producer"),
        }
    }
    fn result_cards(&mut self, verb: Verb, original: bool) -> Res<Slot> {
        let groups: Vec<_> = self.binder.groups.iter().filter(|g| g.by == verb).collect();
        match groups.as_slice() {
            [group] if original => self.binder.discarded.iter().find(|(actual, _)| *actual == group.slot).map(|(_, before)| *before)
                .ok_or_else(|| crate::Unsupported("result quantity: producer has no original-card projection".into())),
            [group] => Ok(group.slot),
            _ => unsupported("result quantity: absent or ambiguous action-result producer"),
        }
    }
    fn player_outcome(&mut self, outcome: ir::condition::Outcome) -> Res<Slot> {
        let [outputs] = self.binder.player_outcomes.as_slice() else {
            return unsupported("player outcome: absent or ambiguous object-payment producer");
        };
        Ok(outputs[match outcome { ir::condition::Outcome::Did => 0, ir::condition::Outcome::Didnt => 1, ir::condition::Outcome::Couldnt => 2 }])
    }
    fn exiled(&mut self) -> Res<Slot> {
        self.binder.exiled.last().copied().ok_or_else(|| crate::Unsupported("other linked cards: no preceding exile result".into()))
    }
    fn entered(&mut self) -> Res<Slot> {
        match self.binder.entered.as_slice() {
            [slot] => Ok(*slot),
            [] => unsupported("movement reference: no earlier battlefield-entry producer"),
            _ => unsupported("movement reference: ambiguous battlefield-entry producers"),
        }
    }
    fn this_way(&mut self, verb: Option<Verb>, into: Option<Zone>) -> Res<Vec<(Slot, expr::Shape, bool)>> {
        if self.binder.unrepresented_movement && verb.is_none_or(|v| v == Verb::Move) {
            return unsupported("result group: complete search-partition movement result not modeled");
        }
        // what destroyed puts each where it went; every other instruction one zone (400.7)
        let groups: Vec<_> = self.binder.produced.iter()
            .filter(|p| verb.is_none_or(|v| p.verb == v) && (into.is_none() || p.shape == expr::Shape::Destroyed || p.into == into))
            .map(|p| (p.slot, p.shape, p.yours)).collect();
        if groups.is_empty() { return unsupported(format!("result group: no earlier instruction did what \"{verb:?} this way\" names")); }
        Ok(groups)
    }
    fn latest_group(&mut self) -> Res<(Slot, expr::Shape)> {
        if self.binder.unrepresented_movement {
            return unsupported("that many: complete search-partition movement result not modeled");
        }
        self.binder.produced.last().map(|p| (p.slot, p.shape)).ok_or_else(|| crate::Unsupported("that many: no preceding group of objects".into()))
    }
    fn condition_amount(&mut self) -> Option<expr::Amount> { self.binder.condition_amount.clone() }
    fn damage(&mut self) -> Res<Slot> {
        match self.binder.damage.as_slice() {
            [slot] => Ok(*slot),
            [] => unsupported("damage this way: no completed producer"),
            _ => unsupported("damage this way: ambiguous completed producers"),
        }
    }
    fn life_lost(&mut self) -> Res<Slot> {
        match self.binder.lost.as_slice() {
            [slot] => Ok(*slot),
            [] => unsupported("life lost this way: no result producer"),
            _ => unsupported("life lost this way: ambiguous result producers"),
        }
    }
}

/// Whether a description can be of players, and of objects.
fn kinds(set: &ObjectSet) -> (bool, bool) {
    let possibilities = set.possibilities();
    (possibilities.iter().any(|p| p.domain.sort == ir::Sort::Player), possibilities.iter().any(|p| p.domain.sort != ir::Sort::Player))
}

impl Binder {
    /// Scope a recipient override to one lowering operation. Nested operations and
    /// refusals restore the same prior context before amounts or later acts bind.
    fn with_recipients<T>(&mut self, context: RecipientContext, lower: impl FnOnce(&mut Self) -> Res<T>) -> Res<T> {
        let previous = std::mem::replace(&mut self.recipients, context);
        let result = lower(self);
        self.recipients = previous;
        result
    }
    /// The paragraph of the sentence being lowered.
    fn paragraph(&self) -> usize { self.paragraphs.iter().filter(|start| **start <= self.at.0).count() }
    fn check_alternative_reference(&self, s: &Selector) -> Res<()> {
        if self.alternative_result && kinds(&s.set).1 && matches!(s.quant, Quant::Definite(Ref::Anaphor(_))) {
            return unsupported("token alternative result references are not implemented");
        }
        Ok(())
    }
    fn scope<'a, 's>(&'a self, support: &'a Support<'s>) -> Scoped<'a, 's> { Scoped { binder: self, support, actor: false, object_actor: false, to_players: false } }
    /// A possessive pronoun in a clause is of that clause's subject where it can be ("It
    /// deals damage equal to its power to target creature"): the antecedent the sentence
    /// gives it, however many other things were named since.
    fn possessor(&self, selector: &Selector) -> Option<expr::Subject> {
        let Quant::Definite(Ref::Anaphor(form)) = selector.quant else { return None };
        let singular = match form {
            ir::selector::Anaphor::It => true,
            ir::selector::Anaphor::Them | ir::selector::Anaphor::They => false,
            _ => return None,
        };
        let (player, object) = kinds(&selector.set);
        if self.object_each && object { return Some(expr::Subject::ObjectActor); }
        let owner = self.clause.as_ref()?;
        let fits = match owner {
            expr::Subject::Source | expr::Subject::Attached => singular && object,
            expr::Subject::Actor => !singular && player,
            expr::Subject::Local(slot) => {
                let (set, max) = self.subjects.iter().find(|n| n.slot == *slot || n.object == Some(*slot)).map(|n| (&n.set, n.bounds.1))
                    .or_else(|| self.result.as_ref().filter(|(s, _, _)| s == slot).map(|(_, set, max)| (set, *max)))?;
                let (p, o) = kinds(set);
                (!singular || max <= 1) && (!p || player) && (!o || object)
            }
            _ => false,
        };
        fits.then(|| owner.clone())
    }
    fn complete_clause(&mut self, captured_source: Option<Slot>) {
        let source = match self.clause {
            Some(expr::Subject::Source) => true,
            Some(expr::Subject::Local(slot)) => captured_source == Some(slot)
                || self.subjects.iter().any(|n| n.slot == slot && n.own && kinds(&n.set) == (false, true)),
            _ => false, // Attached, a controller, a target or another actor replaces Source.
        };
        self.completed_subject = if source { CompletedSubject::Source } else { CompletedSubject::Other };
    }
    fn object(&self, slot: Slot) -> Slot {
        match self.subjects.iter().find(|n| n.slot == slot) {
            Some(named) => named.object.unwrap_or(slot),
            None => self.result_entered.filter(|(result, _)| *result == slot).map_or(slot, |(_, entered)| entered),
        }
    }
    fn name(&mut self, slot: Slot, set: ObjectSet, bounds: (u32, u32), own: bool) {
        self.subjects.push(Named { slot, object: None, set, bounds, target: false, explicit_choice: false, controlled_target: None, attached: false, own, at: self.at });
    }
    /// A root "other creature" set can refer to the target whose controller it names
    /// (608.2c, 615.11). Identity provenance is lexical, never "the last target".
    fn other_target(&self, set: &ObjectSet, support: &Support<'_>) -> Res<Option<expr::Subject>> {
        if !set.other || set.domain != Domain::Permanent { return Ok(None); }
        let Some(relation) = set.controller.as_ref().filter(|r| !r.negated && !r.past) else { return Ok(None); };
        let target = if let Quant::Definite(Ref::ControllerOf(of)) = &relation.who.quant {
            // The root controller relation explicitly names the earlier target, independently
            // of a self-naming clause's characteristic-amount possessive context.
            if !matches!(of.quant, Quant::Definite(Ref::Anaphor(_))) { return Ok(None); }
            match self.referent(of, support, false)? {
                Referent::Local(slots) => match slots.as_slice() {
                    [slot] if self.subjects.iter().any(|n| n.slot == *slot && n.target && n.bounds.1 <= 1) => Some(*slot),
                    _ => None,
                },
                _ => None,
            }
        } else {
            if !matches!(relation.who.quant, Quant::Definite(Ref::Anaphor(_))) { return Ok(None); }
            match expr::Subject::compile(&relation.who, &mut self.scope(support))? {
                expr::Subject::Local(player) => self.subjects.iter().find(|n| n.slot == player).and_then(|n| n.controlled_target),
                _ => None,
            }
        };
        let Some(target) = target else { return Ok(None); };
        let subject = expr::Subject::Local(target);
        // The controller relation identifies an earlier target, while a distributive
        // dealer requires its own "other" referent. Keep this unsupported even when
        // the actual groups happen to be disjoint; comparison referents resolve first.
        if self.recipients.distributive {
            return unsupported("other: earlier-target exclusion under distributive dealers not implemented");
        }
        if self.recipients.subject.as_ref().is_some_and(|actor| *actor != subject) {
            return unsupported("other: ambiguous earlier target and explicit actor");
        }
        Ok(Some(subject))
    }
    /// The complement of one announced target producer (608.2c), only at a clause or
    /// condition root. A plural producer is one slot; separate producers are not a union.
    fn root_other(&self, set: &ObjectSet, support: &Support<'_>) -> Res<Option<expr::Subject>> {
        if !set.other { return Ok(None); }
        if let Some(target) = self.other_target(set, support)? { return Ok(Some(target)); }
        // Preserve the per-dealer recipient boundary before considering any new target.
        if self.recipients.distributive {
            return unsupported("distributive damage: per-source other recipient set not implemented");
        }
        if let Some(actor) = &self.recipients.subject { return Ok(Some(actor.clone())); }
        // An explicitly self-naming acting clause wins over an earlier announced target.
        if self.clause == Some(expr::Subject::Source) { return Ok(Some(expr::Subject::Source)); }
        if let Some((slot, expected)) = &self.contrast && expected == set {
            return Ok(Some(expr::Subject::Local(*slot)));
        }
        let mut targets = Vec::new();
        for named in self.subjects.iter().filter(|n| n.target) {
            // Possibility sorts are type proofs, not current membership. Card types and
            // subtypes may overlap, so different labels do not establish disjointness.
            let compatible = named.set.possibilities().iter().any(|a| set.possibilities().iter().any(|b| {
                (a.domain.sort == ir::Sort::Player) == (b.domain.sort == ir::Sort::Player)
            }));
            if compatible && !targets.contains(&named.slot) { targets.push(named.slot); }
        }
        if !targets.is_empty() {
            let mut plain = set.clone();
            plain.other = false;
            if plain.controller.as_ref().is_some_and(|r| !r.negated && !r.past && crate::select::is_you(&r.who)) { plain.controller = None; }
            let creature = ObjectSet { filter: ir::Filter::All(vec![ir::Filter::Atom(ir::Atom::Type(ir::CardType::Creature))]), ..ObjectSet::of(Domain::Permanent) };
            let plain_creature = plain == creature;
            let compatible = support.completed_source_creature();
            // A lexical spell/Garruk nonmember cannot compete with a plain creature root,
            // even when branch subjects differ. Complex roots are not classified by type.
            if !(plain_creature && compatible == Some(false)) {
                match self.completed_subject {
                    CompletedSubject::Unresolved => return unsupported("other: unresolved completed clause subject"),
                    CompletedSubject::Source if plain_creature && compatible == Some(true) => return Ok(Some(expr::Subject::Source)),
                    CompletedSubject::Source => return unsupported("other: completed source compatibility not implemented"),
                    CompletedSubject::Other => {},
                }
            }
        }
        let created = self.result.as_ref().filter(|(slot, _, _)| set.domain == Domain::Permanent && self.created_groups.contains(slot));
        match (targets.as_slice(), created) {
            ([], Some((slot, _, _))) => Ok(Some(expr::Subject::Local(*slot))),
            ([_ , ..], Some(_)) => unsupported("other: competing target and created-group antecedents"),
            ([], None) => Ok(None),
            ([slot], None) => Ok(Some(expr::Subject::Local(*slot))),
            (_, None) => unsupported("other: ambiguous announced target slots"),
        }
    }
    /// What the reference names: something this text named, else what took part in the
    /// occurrence, else the source the text has spoken of. Decided as the text is lowered,
    /// by how each was named; where that does not decide, the reference is not followed.
    fn referent(&self, selector: &Selector, support: &Support<'_>, acted_on: bool) -> Res<Referent> {
        self.referent_scoped(selector, support, acted_on, false, false)
    }
    /// Bare damage-recipient pronouns have the recipient's object/player signature;
    /// subject and characteristic-possessor references keep their ordinary scope.
    fn referent_scoped(&self, selector: &Selector, support: &Support<'_>, acted_on: bool, recipient: bool, dealer: bool) -> Res<Referent> {
        self.check_alternative_reference(selector)?;
        if let Quant::Definite(Ref::Anaphor(ir::Anaphor::TheParticiple { what: Verb::Choose, plural })) = selector.quant {
            // A noun describes an existing choice; it never reselects by live
            // characteristics. Prove only literal noun/conjunction containment.
            let describes = |said: &ObjectSet| {
                let noun = &selector.set;
                let filter = said.filter == noun.filter || match (&said.filter, &noun.filter) {
                    (_, ir::Filter::All(parts)) if parts.is_empty() => true,
                    (ir::Filter::All(said), ir::Filter::All(noun)) => noun.iter().all(|part| said.contains(part)),
                    _ => false,
                };
                said.domain == noun.domain && filter
                    && noun.controller.as_ref().is_none_or(|r| said.controller.as_ref() == Some(r))
                    && noun.owner.as_ref().is_none_or(|r| said.owner.as_ref() == Some(r))
                    && noun.caster.as_ref().is_none_or(|r| said.caster.as_ref() == Some(r))
                    && noun.zone.as_ref().is_none_or(|r| said.zone.as_ref() == Some(r))
                    && !noun.other
            };
            let candidates: Vec<_> = self.subjects.iter().filter(|n|
                n.explicit_choice && (plural || n.bounds.1 <= 1) && describes(&n.set)).collect();
            if !candidates.is_empty() && input::find(support, selector)?.is_some() {
                return unsupported("chosen reference: ambiguous local and input choices");
            }
            // "Chosen" identifies the declaration, independently of intervening
            // action results (608.2c). It cannot silently select the latest of
            // several compatible declarations or a payment/event participant.
            match candidates.as_slice() {
                [one] => return Ok(Referent::Local(vec![one.slot])),
                [] => {},
                _ => return unsupported("chosen reference: ambiguous explicit choices"),
            }
        }
        if let Quant::Definite(Ref::Anaphor(ir::Anaphor::TheParticiple { what, .. }))=selector.quant {
            // A named payment action supplies its own participant, independently of targets.
            // An earlier body action of the same kind still needs explicit disambiguation.
            if self.groups.iter().any(|g|g.by==what) {
                return unsupported("reference: participial cost/body producers are ambiguous");
            }
            return input::resolve(support,selector).map(Referent::Input);
        }
        let (local, named) = self.local_scoped(selector, recipient)?;
        // "Tap this creature. It gains …": the explicitly named source. What an act
        // is done to is not the source the same act names; "its
        // power" there is ("put X counters on this creature, where X is its power").
        let source = self.source_named.filter(|at| !acted_on || *at != self.at)
            .filter(|_| matches!(selector.quant, Quant::Definite(Ref::Anaphor(ir::Anaphor::It))) && kinds(&selector.set).1);
        // Conservatively decline ordinary Source fallback when a rejected target would
        // win lexical precedence. Plural cardinality takes priority over mixed type.
        // Recipient unions and direct damage-dealer subjects retain their own contracts.
        let (plural, unrefined) = if !recipient && !dealer && kinds(&selector.set) == (false, true) {
            source.map_or((false, false), |source| self.subjects.iter()
                .filter(|n| n.target && !self.source_precedes(source, n.at))
                .fold((false, false), |(plural, unrefined), n| {
                    let (player, object) = kinds(&n.set);
                    (plural || n.bounds.1 > 1 && object,
                     unrefined || n.bounds.1 <= 1 && player && object)
                }))
        } else { (false, false) };
        let source_fallback = || {
            if plural { unsupported("reference: singular reference to a plural target") }
            else if unrefined { unsupported("reference: unrefined mixed target") }
            else { Ok(Referent::Source) }
        };
        if !local.is_empty() {
            // What a clause's subject acts on is not that subject, which would be "itself":
            // of "Garruk deals 3 damage to target creature. That creature deals damage
            // equal to its power to him", the creature deals it to the source the text named.
            if acted_on && source.is_some() && matches!(&self.clause, Some(expr::Subject::Local(slot)) if local == [*slot]) {
                return Ok(Referent::Source);
            }
            // "It" fits something this text named and the source it also named: the
            // antecedent is the one the reference's own sentence named ("If you don't, tap
            // this creature and it deals 2 damage to you" is not said of what an earlier
            // sentence had sacrificed), else the one named later. An act that names both
            // names the source as what acts and the other as what it acts on, and "it"
            // afterwards is the other ("This spell deals 1 damage to target creature. It
            // can't block this turn."): the text says "this creature" of itself.
            let (Some(source), Some(named)) = (source, named) else { return Ok(Referent::Local(local)) };
            return if self.source_precedes(source, named) { source_fallback() }
                else { Ok(Referent::Local(local)) };
        }
        // An explicit mention in this body establishes the antecedent of "it";
        // an event or paid cost supplies it only when the body has not named one.
        if source.is_some() { return source_fallback(); }
        match input::find(support, selector)? {
            Some(input) => Ok(Referent::Input(input)),
            None => input::resolve(support, selector).map(Referent::Input),
        }
    }
    /// The existing lexical source/named precedence, also used for rejected-target proof.
    fn source_precedes(&self, source: (usize, usize), named: (usize, usize)) -> bool {
        let here = |at: (usize, usize)| at.0 == self.at.0 && at.0 != usize::MAX;
        let after = if source.0 == usize::MAX || named.0 == usize::MAX { source.1 > named.1 } else { source > named };
        match (here(source), here(named)) {
            (true, false) => true,
            (false, true) => false,
            _ => after,
        }
    }
    /// Whether the reference is to things this text named that can only be objects.
    fn names_objects(&self, selector: &Selector) -> bool {
        matches!(selector.quant, Quant::Definite(Ref::Anaphor(_))) && self.subject(selector).is_ok_and(|slots| !slots.is_empty()
            && slots.iter().all(|slot| self.subjects.iter().any(|n| n.slot == *slot && kinds(&n.set) == (false, true))))
    }
    /// The slots of what this text named that the reference is said of; none where it
    /// named nothing that fits.
    fn subject(&self, selector: &Selector) -> Res<Vec<Slot>> { Ok(self.local(selector)?.0) }
    /// Those slots, and where the text named them: nowhere for the ability's own object and
    /// for the result of a search or a look, which no mention of the source displaces.
    fn local(&self, selector: &Selector) -> Res<(Vec<Slot>, Option<(usize, usize)>)> {
        self.local_scoped(selector, false)
    }
    fn local_scoped(&self, selector: &Selector, recipient: bool) -> Res<(Vec<Slot>, Option<(usize, usize)>)> {
        let Quant::Definite(Ref::Anaphor(form)) = selector.quant else { return unsupported("reference: unsupported form"); };
        let singular = match form {
            ir::selector::Anaphor::It | ir::selector::Anaphor::That | ir::selector::Anaphor::The { plural: false } => true,
            ir::selector::Anaphor::Those | ir::selector::Anaphor::Them | ir::selector::Anaphor::They | ir::selector::Anaphor::The { plural: true } => false,
            ir::Anaphor::TheParticiple { .. } if !self.subjects.is_empty() => return unsupported("reference: participial reference beside a target or another declared participant is not bound"),
            _ => return unsupported("reference: qualified, reflexive or remainder subject not bound"),
        };
        let (player, object) = kinds(&selector.set);
        // "It" as recipient does not name the dealer (which would require "itself").
        // Exclude it before sentence/noun narrowing, independently of amount possessors.
        let dealer = if recipient { self.clause.as_ref() } else { None };
        let excluded = |slot| matches!(dealer, Some(expr::Subject::Local(s)) if *s == slot);
        let fits = |n: &Named| {
            let (p, o) = kinds(&n.set);
            // A mixed target can denote an object despite its player alternative.
            // Player-only producers are not bare Object-pronoun antecedents.
            !excluded(n.slot) && ((!p || player) && (!o || object) || recipient && n.target && o)
        };
        // What an earlier sentence or a closed conditional branch made is not what this
        // sentence names first: "If this spell was kicked, create a Soldier token. Counter
        // target spell unless its controller pays …": its controller is the spell's. (An act
        // of a conditional sentence has no sentence of its own here, so nothing displaces.)
        let made_elsewhere = |slot: Slot| match self.subjects.iter().find(|n| n.slot == slot) {
            Some(made) => made.at.0 != self.at.0,
            None => self.branch_result == Some(slot),
        };
        let displaced = |slot: Slot| self.at.0 != usize::MAX && made_elsewhere(slot)
            && self.subjects.iter().any(|n| n.slot != slot && n.at.0 == self.at.0 && !n.own && fits(n) && (!singular || n.bounds.1 <= 1));
        if let Some((slot, set, max)) = &self.result
            && !excluded(*slot) && !displaced(*slot)
        {
            let (p, o) = kinds(set);
            if (!singular || *max <= 1 || self.created_single == Some(*slot)) && (!p || player) && (!o || object) { return Ok((vec![*slot], None)); }
        }
        let mut candidates: Vec<&Named> = self.subjects.iter().filter(|n| {
            fits(n) && (!singular || n.bounds.1 <= 1)
        }).collect();
        if recipient && singular && candidates.iter().all(|n| n.own)
            && self.subjects.iter().any(|n| fits(n) && n.target && n.bounds.1 > 1)
        {
            return unsupported("damage recipient: singular reference to a plural producer");
        }
        // Each step keeps what the grammar of the text points to, while anything is left:
        // not the ability's own object, which the text calls "it" or "this creature" ("put
        // a counter on it and on target creature. That creature …"); for a plural, what
        // was named as several; what was called by the same noun ("tap target land. That
        // land …"); what the reference's own sentence named ("tap target creature, and
        // that creature …"). Never the nearest: several left is a reference not followed.
        let narrow = |candidates: &mut Vec<&Named>, keep: &dyn Fn(&Named) -> bool| {
            if candidates.len() > 1 && candidates.iter().any(|n| keep(n)) { candidates.retain(|n| keep(n)); }
        };
        narrow(&mut candidates, &|n| !n.own);
        if !singular {
            // "untap target creature and each other creature that shares a color with it.
            // Those creatures get +2/+0": what one act named together is not split in two
            let several = |slot: Slot| candidates.iter().any(|m| m.slot == slot && m.bounds.1 > 1);
            if candidates.iter().any(|n| n.bounds.1 <= 1
                && self.coordinated.iter().any(|group| group.contains(&n.slot) && group.iter().any(|s| several(*s))))
            {
                return unsupported("reference: a plural of one act's single and several objects");
            }
            narrow(&mut candidates, &|n| n.bounds.1 > 1);
        }
        // A definite qualified noun identifies a declared participant. By contrast,
        // "those/that creatures you don't control" may restrict an existing group
        // through the action's current-state filter (e.g. Lost in the Maze).
        let qualified_noun = matches!(form, ir::Anaphor::The { plural: false }) && selector.set.controller.is_some();
        if qualified_noun && !candidates.is_empty() {
            candidates.retain(|n| {
                if n.set.controller == selector.set.controller { return true; }
                // The later definite opponent-qualified noun can identify the exact
                // previously declared opposing Fight participant (608.2c). This is
                // lexical proof, not a fresh current-controller query after damage.
                if !self.fight_recipients.contains(&n.slot) { return false; }
                let (Some(later), Some(earlier)) = (&selector.set.controller, &n.set.controller) else { return false };
                if later.negated || later.past || earlier.negated || earlier.past
                    || later.who.quant != Quant::Definite(Ref::Anaphor(ir::Anaphor::The { plural: false }))
                    || !crate::select::is_an_opponent(&earlier.who) { return false; }
                let mut who = (*later.who).clone();
                who.quant = earlier.who.quant.clone();
                who == *earlier.who
            });
            if candidates.is_empty() { return unsupported("reference: controller qualification has no matching earlier participant"); }
        }
        // A qualified noun ("the creature an opponent controls") names the
        // matching earlier participant, not a different creature with the same type.
        if qualified_noun { narrow(&mut candidates, &|n| n.set == selector.set); }
        // "That permanent" distinguishes an ability's named source from the
        // earlier stack ability, even when the source was qualified by its types.
        if matches!(selector.set.domain, Domain::Permanent | Domain::Ability | Domain::Spell) {
            narrow(&mut candidates, &|n| n.set.domain == selector.set.domain);
        }
        narrow(&mut candidates, &|n| n.set.domain == selector.set.domain && n.set.filter == selector.set.filter);
        narrow(&mut candidates, &|n| n.at.0 == self.at.0);
        // A body explicitly naming the attached creature supplies the pronoun's
        // antecedent before an unnamed occurrence participant (e.g. the creature
        // that died in Blade of the Bloodchief's trigger). Keep ambiguity with
        // another explicit body participant instead of using nearest-mention order.
        if candidates.iter().any(|n| n.attached)
            && candidates.iter().all(|n| n.attached || n.at.0 == usize::MAX)
        {
            narrow(&mut candidates, &|n| n.attached);
        }
        match candidates.as_slice() {
            [] => Ok((Vec::new(), None)),
            [one] => Ok((vec![one.slot], (!one.own).then_some(one.at))),
            // a plural names together what one act named together
            [first, rest @ ..] if !singular && rest.iter().all(|n| n.at == first.at) => Ok((candidates.iter().map(|n| n.slot).collect(), Some(first.at))),
            _ => unsupported("reference: ambiguous earlier subject producers"),
        }
    }
    fn scalar(&self, r: &UnresolvedRef, support: &Support<'_>) -> Res<Scalar> {
        let locals: Vec<_> = self.scalars.iter().filter(|(_, kind)| crate::binding::fits(r.expected, *kind)).collect();
        let linked = support.choice_candidates(r.expected)?;
        if locals.len() + linked.len() > 1 {
            return unsupported("choice reference: ambiguous local or linked producers");
        }
        if let [(slot, kind)] = locals.as_slice() { return Ok(Scalar::Local { slot: *slot, kind: *kind }); }
        let (link, kind) = support.choice(r.expected)?;
        Ok(Scalar::Linked { link, kind })
    }
    fn selection(&self, set: &ObjectSet, support: &Support<'_>) -> Res<expr::Cards> {
        expr::Cards::compile(set, support, &mut self.scope(support))
    }

    /// The one group the reference can mean. No group in scope, or several that fit, is an
    /// error: there is no "latest" fallback.
    fn resolve(&mut self, r: &UnresolvedRef) -> Res<&mut Group> {
        if r.expected != Kind::Cards {
            return unsupported("a reference to something other than a group of cards");
        }
        let mut fits: Vec<usize> = (0..self.groups.len())
            .filter(|i| r.qualifiers.iter().all(|Qualifier::ThisWay(verb)| self.groups[*i].by == *verb))
            .collect();
        // of several, the one its own paragraph produced: a paragraph is said apart (608.2c)
        let paragraph = self.paragraph();
        if fits.len() > 1 && let [one] = fits.iter().copied().filter(|i| self.groups[*i].paragraph == paragraph).collect::<Vec<_>>().as_slice() {
            fits = vec![*one];
        }
        match fits[..] {
            [one] => Ok(&mut self.groups[one]),
            [] => unsupported(format!("reference {:?} ({:?}): no earlier instruction produces what it names", r.form, r.qualifiers)),
            _ => unsupported(format!("reference {:?} ({:?}): more than one earlier group could be meant", r.form, r.qualifiers)),
        }
    }
}

// ---------------------------------------------------------------- the compiler

/// A plain top slice and the player whose library supplies it.
/// How a card manifested or cloaked is face down (701.40a, 701.58a).
fn concealment(verb: ir::action::ObjectAction) -> crate::face::Concealment {
    if verb == ir::action::ObjectAction::Cloak { crate::face::Concealment::Cloaked } else { crate::face::Concealment::Manifested }
}

fn top_of_library(s: &Selector) -> Option<(&Amount,&Selector)> {
    let set=&s.set;let zone=set.zone.as_ref()?;
    let plain=set.domain==Domain::Card && set.filter==ir::Filter::All(Vec::new())
        && set.controller.is_none() && set.owner.is_none() && set.caster.is_none()
        && s.chooser.is_none() && s.random.is_none();
    match &zone.slice {
        Some(Slice::Top(n)) if s.quant==Quant::The && plain && zone.zones.items==[ir::Zone::Library] => Some((n,zone.owner.as_deref()?)),
        _=>None,
    }
}

/// Card movements use their owner's corresponding zone (400.3). A stated player still
/// has to bind correctly; "back" refers to the library slice already being processed.
fn destination(to: &Destination, from: Zone, support: &Support<'_>, binder: &Binder) -> Res<Dest> {
    let owner = |whose: &Whose| -> Res<()> { match whose {
        Whose::You | Whose::Owner | Whose::Owners => Ok(()),
        Whose::Player(who) => expr::Subject::compile(who,&mut binder.scope(support)).map(|_|()),
        _=>unsupported("destination: unbound possessive player"),
    }};
    match to {
        Destination::Hand(who) => {owner(who)?;Ok(Dest::Hand)},
        Destination::Graveyard(who) => {owner(who)?;Ok(Dest::Graveyard)},
        Destination::Library(ir::LibraryPlace { library, back, position: ir::Position::One(position), order }) => {
            if let Some(who)=library {owner(who)?;}
            else if !back || from!=Zone::Library {return unsupported("library destination has no bound owner");}
            Ok(Dest::Library { position:*position,order:*order })
        }
        other=>unsupported(format!("destination of a revealed card: {}",crate::select::name(other))),
    }
}

/// Entry modifiers share one lowering path for named objects and selected card groups.
fn battlefield_destination(entries: &[ir::action::Entry], support: &Support<'_>, binder: &Binder) -> Res<action::Destination> {
    let (mut owner_controls, mut tapped) = (false, false);
    let mut counters = Vec::new();
    let mut attacking = None;
    let mut transformed = false;
    for entry in entries {
        match entry {
            ir::action::Entry::Tapped => tapped = true,
            ir::action::Entry::Transformed => transformed = true,
            ir::action::Entry::AttackingSameAs(s) => {
                if attacking.replace(input::cost_defender(support, s)?).is_some() { return unsupported("entry has multiple defenders"); }
            },
            ir::action::Entry::WithCounters(with) => {
                let c = &with.counters;
                if c.up_to || c.extra.is_some() { return unsupported("return entry: optional or additional counter count"); }
                crate::layers::counter_ok(&c.kind.0)?;
                counters.push((c.kind.0.clone(), quantity(&c.count, support, binder)?));
            },
            ir::action::Entry::Under(Whose::Owner | Whose::Owners) => owner_controls = true,
            ir::action::Entry::Under(Whose::You) => owner_controls = false,
            _ => return unsupported(format!("battlefield entry modifier not lowered: {entry:?}")),
        }
    }
    // put there by the player whose clause it is (110.2a); by each player, what they found
    // in their own library, so under its owner's control
    let owner_controls = owner_controls || binder.each_does;
    Ok(action::Destination::Battlefield { owner_controls, by: binder.actor.map_or(Who::Controller, Who::Of), tapped, counters, attacking, transformed, face_down: None })
}

/// What the grammar reads of an act but no instruction does: refused by name before its
/// subject or modifiers are asked, so the refusal says what is not run rather than where
/// lowering stopped.
fn admitted(act: &ir::Act) -> Res<()> {
    match &act.action {
        Action::DrawUpTo(n) if !matches!(n, Amount::Const(_)) => unsupported("draw up to: fixed numeric bound required"),
        // what the turn plan does not hold: a skipped turn, every combat phase of a turn, an
        // added ending phase, which needs a cleanup continuation (500.8)
        Action::Static(s) if matches!(**s, ir::StaticAct::Skip(ir::statics::Skipped::ThatTurn)) => unsupported("skipping a named turn (614.10)"),
        Action::Static(s) if matches!(**s, ir::StaticAct::Skip(ir::statics::Skipped::CombatPhasesOfNextTurn)) => unsupported("skipping every combat phase of a turn (614.10a)"),
        Action::Static(s) if matches!(**s, ir::StaticAct::Skip(ir::statics::Skipped::Step { next: false, .. })) => unsupported("skipping a step that is not the next one (614.10)"),
        Action::AdditionalPhase { kind: ir::turn::Phase::Ending, .. } => unsupported("adding an ending phase (500.8; cleanup continuation not implemented)"),
        Action::Create(tokens) if tokens.items.len() > 1 && tokens.conj != ir::Conj::And && tokens.conj != ir::Conj::Or => unsupported("unsupported token connective"),
        Action::DealDamage(hits) => hits.iter().flat_map(|h| &h.to.items).try_for_each(|to| crate::event::damage_recipient_ok(&to.set)),
        _ => Ok(()),
    }
}

/// "Repeat this process" is lowered where the process is known to stop, as a mandatory loop
/// that cannot stop is a draw (104.4b, 732.4) and no rule caps it. The process reveals the
/// top card of its controller's library and, only if that card is as described, puts it
/// into a graveyard. Each time through that goes on takes one card out of a finite library,
/// and no admitted replacement keeps it there or puts it back (one that does, like Wheel of
/// Sun and Moon, is refused); a reveal of an empty library shows nothing, and nothing is as
/// described (Countryside Crusher).
fn repeated_process(instrs: &[Instr]) -> Res<()> {
    let stops = match instrs {
        [Instr::TakeTop { n, whose: action::Operand::Controller, out: taken },
         Instr::Show { from: shown_from, to: Audience::Everyone, out: shown, .. },
         Instr::Subjects { cards, out: subjects, still_in: None },
         Instr::Branch { condition, .. },
         Instr::Action { action: action::Action::Move { groups, .. }, created: _, attempt: Some(attempt) }] =>
            *n == expr::Amount::constant(1) && shown_from == taken && cards == shown && condition.is_of(*subjects)
                && !attempt.optional && !attempt.payment
                && matches!(groups.as_slice(), [(moved, action::Destination::Zone(Zone::Graveyard))]
                    if moved.as_slice() == [action::Operand::Reference(*subjects)]),
        _ => false,
    };
    if stops { Ok(()) } else { unsupported("repeat this process: only a revealed top card put into a graveyard while it is as described") }
}

/// An act no instruction does, named: a keyword action on objects by the action itself.
fn not_lowered(action: &Action) -> crate::Unsupported {
    let name = match action {
        Action::OnObjects(verb, _) => crate::select::name(verb),
        other => crate::select::name(other),
    };
    crate::Unsupported(format!("program: action not lowered: {name}"))
}

/// "You gain protection from everything until your next turn": keywords given to players,
/// kept as the rule they are of them ([`crate::restrict::Rule::compile`]), not as a change
/// to objects' characteristics.
fn player_grant(subject: Option<&Subject>, action: &Action) -> bool {
    fn players(subject: &Subject) -> bool {
        let set = match subject {
            // a bare "they" is read as players, but names the permanents before it where it
            // can (the continuous path below)
            Subject::Selector(s) | Subject::Each(s) if matches!(s.quant, Quant::Definite(Ref::Anaphor(_))) => return false,
            Subject::Selector(s) | Subject::Each(s) => &s.set,
            Subject::Set(set) => set,
            Subject::And { items, .. } => return items.iter().all(players),
            Subject::LifeTotal(_) => return false,
        };
        let possible = set.possibilities();
        !possible.is_empty() && possible.iter().all(|p| p.domain.sort == ir::Sort::Player)
    }
    matches!(action, Action::Gains(granted) if !granted.is_empty() && granted.iter().all(|g| matches!(g, ir::Granted::Keyword(_))))
        && subject.is_some_and(players)
}

/// "Gains protection from the color of your choice": its controller chooses the color as
/// it resolves, just before the effect begins (608.2h), and the grant reads the chosen
/// color. The grant as it reads it, if it chooses one.
fn choose_protection_color(e: &mut Emit, binder: &mut Binder, action: &Action) -> Option<Action> {
    let Action::Gains(granted) = action else { return None };
    if !granted.iter().any(|g| matches!(g, ir::Granted::Keyword(k) if crate::protection::choice_of_color(k).is_some())) { return None; }
    let out = e.slot();
    e.instrs.push(Instr::Choose { kind: Kind::Color.into(), who: Who::Controller, out });
    binder.scalars.push((out, Kind::Color));
    Some(Action::Gains(granted.iter().map(|g| match g {
        ir::Granted::Keyword(k) => ir::Granted::Keyword(Box::new(crate::protection::choice_of_color(k).unwrap_or_else(|| (**k).clone()))),
        other => other.clone(),
    }).collect()))
}

/// What "[do this] unless [a player does that]" asks of the player (118.12a): a cost of mana
/// and life, paid as a whole, or one action of objects that player chooses, done or not.
enum Unless {
    Cost { said: ir::Cost, each: Option<Amount> },
    Objects(Action),
}

impl Unless {
    fn compile(actions: &[Action]) -> Res<Unless> {
        if let [action] = actions && !matches!(action, Action::PayMana(_) | Action::PayLife(_) | Action::Waterbend(_)) {
            return match action {
                Action::Sacrifice(_) | Action::Discard(_) | Action::Exile(_) | Action::Tap(_) => Ok(Unless::Objects(action.clone())),
                Action::Move { moves, from: None, .. } if matches!(moves.as_slice(), [(_, Destination::Hand(Whose::Owner))]) => Ok(Unless::Objects(action.clone())),
                _ => unsupported(format!("unless: a payment of {}", crate::select::name(action))),
            };
        }
        let parts = actions.iter().map(|a| match a {
            Action::PayMana(mana) => Ok(ir::CostPart::Mana(mana.clone())),
            // "unless you waterbend {2}", "Ward—Waterbend {4}" (701.67a)
            Action::PayLife(_) | Action::Waterbend(_) => Ok(ir::CostPart::Action(a.clone())),
            _ => unsupported("unless: a payment other than mana, life or a waterbend cost"),
        }).collect::<Res<Vec<_>>>()?;
        Ok(Unless::Cost { said: ir::Cost(parts), each: None })
    }
}

fn controller_only(subject: Option<&Subject>) -> bool {
    match subject {
        None => true,
        Some(Subject::Selector(s)) => s.quant == Quant::Definite(Ref::You) && s.chooser.is_none() && s.random.is_none(),
        Some(_) => false,
    }
}

/// A single actor may make a payment. Multi-player optional payments need per-player
/// outcomes rather than one consent bit; they are not admitted by this form.
/// "Their" of a destination names a player the text supplies: the one the clause is said of,
/// or one named before it ("look at target player's library ... into their graveyard"). With
/// none, nothing says whose it is (400.3). "You" is never "their": in "You shuffle their
/// graveyard into their library" it is the player named before.
fn their(e: &mut Emit, clause: &Clause, support: &Support<'_>, binder: &mut Binder) -> Res<action::Operand> {
    if clause.subject.is_some() && single_actor(clause.subject.as_ref()) && !controller_only(clause.subject.as_ref()) {
        return e.clause_subject(clause, support, binder);
    }
    let they = Selector::new(Quant::Definite(Ref::Anaphor(ir::Anaphor::They)), ObjectSet::of(Domain::Player));
    e.operand(&they, support, binder)
}

fn single_actor(subject: Option<&Subject>) -> bool {
    match subject {
        None => true,
        Some(Subject::Selector(s)) => matches!(&s.quant,
            Quant::Definite(Ref::You | Ref::DefendingPlayer | Ref::ControllerOf(_) | Ref::OwnerOf(_) | Ref::Anaphor(ir::Anaphor::That | ir::Anaphor::It | ir::Anaphor::The { plural: false }))
            | Quant::Target(Count::Exactly(Amount::Const(1)) | Count::UpTo(Amount::Const(1)))),
        _ => false,
    }
}

/// Structured text flattened into instructions with explicit lexical scope boundaries.
#[derive(Clone, Copy)]
/// A delayed or reflexive ability's step ends with the sentence its lead begins, whole: the
/// start of the text that ability has (603.7, 603.12).
enum TextStep<'a> { Reflexive(&'a Sentence, &'a [Sentence], Option<&'a ir::EventPattern>, &'a Sentence),
    /// "When that mana is spent to cast …, scry 1.": what it is spent on, what follows the
    /// lead, and the whole sentence, which is the delayed ability's text (106.6, 603.7a)
    ManaSpent(&'a ir::mana::SpendOn, &'a Sentence, &'a Sentence), Alternative(&'a Clause), May(&'a Clause), EndMay, Delayed(&'a ir::EventPattern, Option<ir::DelayLimit>, bool, &'a Sentence, &'a Sentence),
    /// "At the beginning of your first upkeep, look at …. … .": what follows the lead and the
    /// sentences after it, all done then (only what an opening-hand action has done, 103.6)
    Later(ir::event::Step, &'a ir::event::StepOwner, &'a Sentence, &'a [Sentence], &'a Sentence), Act(&'a Clause, &'a ir::effect::Act, bool), If(&'a ir::Condition, bool), EndIf, Otherwise, Instead(bool), Duration(&'a ir::effect::Duration), EndDuration, Rule(&'a ir::statics::Static), Each(&'a ObjectSet, bool), EndEach, AfterTurn(ir::effect::TurnRef), EndAfterTurn, DuringCreatedTurn, EndDuringCreatedTurn,
    /// "If an instant or sorcery spell cast this way would be put into your graveyard, exile it
    /// instead": what the cast offered just before carries.
    CastRider(&'a ir::statics::Static),
    /// "If that creature would die this turn, exile it instead.": a replacement kept for the
    /// objects it names (614.1a).
    ExiledInstead(&'a Selector),
    /// One optional retained-card selection for each card type, committed together.
    CardTypes(&'a Sentence),
    CardPayments(&'a ObjectSet, &'a Sentence),
    /// A cost-component rider attached to the body's one proved mana producer.
    ManaGenericForbidden,
    /// A keyword cost defined by the recipient's own mana cost (702.34a).
    KeywordCost(ir::Keyword, &'a Selector) }

/// "If an instant or sorcery spell cast this way would be put into your graveyard, exile it
/// instead": the card types of the spells cast under the offer before it that are exiled
/// instead of being put into their owner's graveyard (614.1a).
fn cast_rider(rule: &ir::statics::Static) -> Option<Vec<ir::CardType>> {
    let ir::statics::Static::Replacement(r) = rule else { return None };
    if r.next_time || r.while_.is_some() || r.this_turn || r.instead_first { return None; }
    let p = r.single_event()?;
    if p.counted() || p.during.is_some() || p.condition.is_some() || p.cause.is_some() { return None; }
    let ([s], [ir::EventAction::PutIntoGraveyard { into, from: None }]) = (p.subjects.as_slice(), p.actions.as_slice()) else { return None };
    if !crate::select::is_you(into) || !matches!(s.quant, Quant::Indefinite(Count::Exactly(Amount::Const(1))) | Quant::Definite(Ref::Anaphor(ir::Anaphor::That))) || s.set.domain != Domain::Spell
        || s.set != (ObjectSet { filter: s.set.filter.clone(), ..ObjectSet::of(Domain::Spell) }) || s.chooser.is_some() || s.random.is_some() { return None; }
    // "an instant or sorcery spell", "cast this way"
    let ir::Filter::All(atoms) = &s.set.filter else { return None };
    let this_way = ir::Filter::Atom(Atom::Reference(UnresolvedRef { expected: Kind::Cards, form: RefForm::ActionResult, qualifiers: vec![Qualifier::ThisWay(Verb::Cast)] }));
    let referred = s.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::That));
    let type_of = |f: &ir::Filter| match f { ir::Filter::Atom(Atom::Type(t)) => Some(*t), _ => None };
    let types = if referred && atoms.is_empty() { Vec::new() } else {
        let [types, cast] = atoms.as_slice() else { return None };
        if *cast != this_way { return None; }
        match types {
        ir::Filter::Any(arms) => arms.iter().map(type_of).collect::<Option<Vec<_>>>()?,
        one => vec![type_of(one)?],
    }};
    // "exile it instead"
    let ir::statics::Substitute::Body(body) = &r.with else { return None };
    let ir::effect::Body { first: Clause { subject: None, may: false, also: false, first: ir::effect::Act { action: Action::Exile(it), mods, may: false }, rest }, rest: more } = body.as_ref() else { return None };
    let [it] = it.items.as_slice() else { return None };
    (matches!(it.quant, Quant::Definite(Ref::Anaphor(ir::Anaphor::It))) && matches!(mods.as_slice(), [ir::effect::Modifier::Instead]) && rest.is_empty() && more.is_empty())
        .then_some(types)
}

/// Is the condition said of something the text names ("if it's blue", "if that creature is
/// tapped", "if that opponent controls …"), now or as it was?
fn said_of_named(c: &ir::Condition) -> bool {
    match c {
        ir::Condition::Is { what, .. } => matches!(what.quant, Quant::Definite(Ref::Anaphor(_))),
        ir::Condition::Was(before) => said_of_named(before),
        ir::Condition::Controls { player, .. } => matches!(player.quant, Quant::Definite(Ref::Anaphor(_))),
        _ => false,
    }
}

/// Is every act of this clause one that says who does it, so that another player than the
/// controller can be the one who does them all ("Its controller may search their library for
/// a basic land card, put that card onto the battlefield tapped, then shuffle.")? A search of
/// that player's own library ("their"), revealing/putting what was found, and a shuffle. Any
/// other act is still the controller's alone to do here.
/// The zones a search looks through and whose they are: one player's library, or several of
/// their zones at once ("its controller's graveyard, hand, and library"), each looked at whole
/// (701.23a). Any other zone or part of one is not searched here.
fn searched_zones(zone: &ir::selector::ZoneSpec) -> Option<(&Selector, Vec<Zone>)> {
    let owner = zone.owner.as_deref().filter(|_| zone.slice.is_none() && zone.zones.conj != ir::action::Conj::Or)?;
    let zones = zone.zones.items.iter().map(|z| match z {
        ir::Zone::Library => Some(Zone::Library),
        ir::Zone::Graveyard => Some(Zone::Graveyard),
        ir::Zone::Hand => Some(Zone::Hand),
        _ => None,
    }).collect::<Option<Vec<_>>>()?;
    let alone = |z: Zone| zones.iter().filter(|x| **x == z).count() == 1;
    (zones.contains(&Zone::Library) && zones.iter().all(|z| alone(*z))).then_some((owner, zones))
}

fn done_by_its_player(clause: &Clause) -> bool {
    // one player, who does it all: an object's controller or owner ("Its controller", "That
    // land's controller") or one targeted player; never several ("each opponent may …")
    let one = matches!(clause.subject.as_ref(), Some(Subject::Selector(s)) if matches!(s.quant, Quant::Definite(Ref::ControllerOf(_) | Ref::OwnerOf(_) | Ref::Anaphor(ir::Anaphor::That)))
        || (s.set.domain == Domain::Player && s.quant == Quant::Target(Count::Exactly(Amount::Const(1)))));
    one && searches_their_own(clause)
}

/// "Each player searches their library for a basic land card, puts it onto the battlefield,
/// then shuffles.": the players each of whom does the whole clause, of their own library
/// and of what they found in it (701.23i). Never as an option: one consent each is not held.
fn done_by_each_player(clause: &Clause) -> Option<&ObjectSet> {
    let Some(Subject::Selector(s)) = clause.subject.as_ref() else { return None };
    let each = matches!(s.quant, Quant::Each | Quant::All) && s.set.domain == Domain::Player && s.chooser.is_none() && s.random.is_none();
    (each && !clause.may && searches_their_own(clause)
        && matches!(clause.first.action, Action::Search { .. })).then_some(&s.set)
}

/// Every act of the clause one its player does of their own library and what they find in
/// it: searching "their library", putting what was found somewhere, revealing it, shuffling.
fn searches_their_own(clause: &Clause) -> bool {
    let theirs = |owner: &Selector| matches!(owner.quant, Quant::Definite(Ref::Anaphor(ir::Anaphor::They)));
    // what was found: "that card", "it", "those cards", "them"
    let found = |s: &Selector| mention(s).is_some() || matches!(s.quant, Quant::Definite(Ref::Anaphor(
        ir::Anaphor::That | ir::Anaphor::It | ir::Anaphor::Those | ir::Anaphor::Them | ir::Anaphor::The { plural: true })));
    std::iter::once(&clause.first).chain(clause.rest.iter().map(|(_, a)| a)).all(|act| !act.may && act.mods.is_empty() && match &act.action {
        Action::Search { zone, .. } => zone.owner_of(ir::Zone::Library).is_some_and(theirs),
        // "put that card onto the battlefield", "put them into their hand"
        Action::Move { moves, from: None, .. } => moves.iter().all(|(objects, _)| objects.items.iter().all(found)),
        Action::Reveal(objects) => objects.items.iter().all(found),
        Action::Shuffle(ir::procedure::Shuffled::Default) => true,
        _ => false,
    })
}

/// A found card can stay aside during a shuffle when the next action places it on
/// every control-flow path (701.24b). Conditions inspect the state without mutating it.
fn following_destination(steps: &[TextStep<'_>]) -> bool {
    let skip_end = |s: &TextStep<'_>| matches!(s, TextStep::EndMay | TextStep::EndDuration | TextStep::EndIf);
    let steps = &steps[steps.iter().take_while(|s| skip_end(s)).count()..];
    let moves = |s: &TextStep<'_>| matches!(s, TextStep::Act(_, ir::effect::Act { action: Action::Move { .. }, .. }, _));
    match steps {
        [first, ..] if moves(first) => true,
        [TextStep::If(_, _), yes, TextStep::EndIf, TextStep::Otherwise, no, TextStep::EndIf, ..] => moves(yes) && moves(no),
        _ => false,
    }
}

/// "If a permanent dealt damage by ~ would die this turn" said by an instant or sorcery: every
/// permanent dealt damage by this spell, its "this turn" the replacement's. Only a spell: it
/// deals no damage after it resolves, as a permanent named so would.
fn dealt_damage_by_this(who: &Selector, support: &Support<'_>) -> Option<Selector> {
    if !support.spell_text() || !matches!(who.quant, Quant::Indefinite(Count::Exactly(Amount::Const(1)))) || who.chooser.is_some() || who.random.is_some() { return None; }
    let by_this = |a: &ir::Filter| matches!(a, ir::Filter::Atom(Atom::DealtDamageBy { by, this_turn: false }) if by.quant == Quant::Definite(Ref::This));
    let mut all = Selector { quant: Quant::All, ..who.clone() };
    let atom = match &mut all.set.filter {
        f if by_this(f) => f,
        ir::Filter::All(parts) => parts.iter_mut().find(|f| by_this(f))?,
        _ => return None,
    };
    if let ir::Filter::Atom(Atom::DealtDamageBy { this_turn, .. }) = atom { *this_turn = true; }
    Some(all)
}

/// "If [condition], [one act] instead.": the condition of a sentence that replaces the one
/// before it.
fn instead(sentence: &Sentence) -> Option<&ir::Condition> {
    let Sentence::Led { lead: Lead::Modifier(ir::effect::Modifier::If(condition)), rest } = sentence else { return None };
    // "If ~ has a luck counter on it, instead add one mana of any color.": said before the act
    let (rest, led) = match &**rest {
        Sentence::Led { lead: Lead::Modifier(ir::effect::Modifier::Instead), rest } => (&**rest, true),
        rest => (rest, false),
    };
    let Sentence::Do(body) = rest else { return None };
    // "If this spell was bargained, instead it deals 3 damage to that permanent and you scry
    // 1.": an "instead" that leads the sentence is said of all of it; one said after an act,
    // only of that act
    let alone = body.rest.is_empty() && body.first.rest.is_empty();
    (led || alone && body.first.first.mods.contains(&ir::effect::Modifier::Instead)).then_some(condition)
}

/// An "instead" sentence that declares targets of its own: "If the gift was promised, instead
/// return target nonland permanent an opponent controls to its owner's hand."
fn alternative_targets(sentence: &Sentence) -> bool {
    // each selector's quantity is printed as its `quant` field; only a target's is `Target`
    format!("{sentence:?}").contains("quant: Target(")
}

/// The sentence an "instead" sentence says is done instead ([`instead`]), without its lead.
fn done_instead(sentence: &Sentence) -> &Sentence {
    let Sentence::Led { rest, .. } = sentence else { unreachable!("the sentence that says instead") };
    match &**rest {
        Sentence::Led { lead: Lead::Modifier(ir::effect::Modifier::Instead), rest } => rest,
        rest => rest,
    }
}

/// "Target opponent reveals their hand. You choose a nonland card from it and exile that card.
/// / Delirium — If …, search that player's graveyard, hand, and library for … the exiled
/// card …": an instant's or sorcery's paragraphs are its instructions, followed in the order
/// written (608.2c), so one may name what one before it did. A paragraph after another is
/// lowered with it, as its later sentences; not a modal one, nor one that replaces the one
/// before it ([`replaces_previous`]).
pub(crate) fn continues_previous(def: &crate::card::CardDef, i: usize) -> bool {
    let plain = |at: usize| match def.abilities.get(at) {
        Some(ability @ ir::Ability::Text(Effect(sentences))) => def.modes_of(at).is_empty() && crate::functions::spell_instructions(ability)
            && !sentences.iter().any(|s| matches!(s, Sentence::ChooseModes { .. })) && replaces_previous(ability).is_none(),
        _ => false,
    };
    // a split card's second half begins its own instructions, as its own spell (709.3)
    let half_begins = def.halves.first().is_some_and(|left| i == left.abilities.len());
    (def.is(ir::CardType::Instant) || def.is(ir::CardType::Sorcery)) && i > 0 && !half_begins && plain(i - 1) && plain(i)
}

/// "Metalcraft — This spell deals 4 damage instead if you control three or more artifacts.":
/// a paragraph that says only what is done instead of the paragraph before it. It is that
/// paragraph's sentence, said after it: "If you control three or more artifacts, this spell
/// deals 4 damage instead."
pub(crate) fn replaces_previous(ability: &ir::Ability) -> Option<Sentence> {
    let ir::Ability::Text(Effect(sentences)) = ability else { return None };
    if let [sentence] = sentences.as_slice() && instead(sentence).is_some() { return Some(sentence.clone()); }
    let [Sentence::Do(body)] = sentences.as_slice() else { return None };
    if !body.rest.is_empty() || !body.first.rest.is_empty() { return None; }
    let mods = &body.first.first.mods;
    // The final "instead if" controls whether this paragraph replaces the previous one.
    // Earlier guards still govern its action: "Destroy that creature if ... instead if ...".
    let (condition, retained) = match mods.as_slice() {
        [ir::effect::Modifier::If(c), ir::effect::Modifier::Instead]
        | [ir::effect::Modifier::Instead, ir::effect::Modifier::If(c)] => (c, vec![ir::effect::Modifier::Instead]),
        [guards @ .., ir::effect::Modifier::Instead, ir::effect::Modifier::If(c)]
            if guards.iter().all(|m| matches!(m, ir::effect::Modifier::If(_))) => {
                let mut retained = guards.to_vec();
                retained.push(ir::effect::Modifier::Instead);
                (c, retained)
            }
        _ => return None,
    };
    let mut said = body.clone();
    said.first.first.mods = retained;
    Some(Sentence::Led { lead: Lead::Modifier(ir::effect::Modifier::If(condition.clone())), rest: Box::new(Sentence::Do(said)) })
}

fn text_steps<'a>(effect: &'a Effect, support: &Support<'_>) -> Res<Vec<TextStep<'a>>> {
    fn walk<'a>(sentence: &'a Sentence, out: &mut Vec<TextStep<'a>>) -> Res<()> {
        match sentence {
            Sentence::Do(body) => {
                if body.rest.iter().any(|(join, _)| *join == Join::Or) { return unsupported("program: alternative clauses not lowered"); }
                for clause in std::iter::once(&body.first).chain(body.rest.iter().map(|(_, c)| c)) {
                    if !clause.also && !clause.rest.is_empty() && clause.rest.iter().all(|(join,_)| *join == Join::Or) {
                        out.push(TextStep::Alternative(clause));
                        continue;
                    }
                    // "it also gains trample": done as said, in addition to what came before
                    if clause.rest.iter().any(|(join, _)| *join == Join::Or) {
                        return unsupported("program: clause connective not lowered");
                    }
                    let alone = clause.rest.is_empty();
                    if let Some(players) = done_by_each_player(clause) {
                        out.push(TextStep::Each(players, true));
                        for act in std::iter::once(&clause.first).chain(clause.rest.iter().map(|(_, a)| a)) {
                            out.push(TextStep::Act(clause, act, alone));
                        }
                        out.push(TextStep::EndEach);
                        continue;
                    }
                    if clause.may && !alone {
                        if !controller_only(clause.subject.as_ref()) && !done_by_its_player(clause) {
                            return unsupported("optional compound instruction by another chooser not lowered");
                        }
                        out.push(TextStep::May(clause));
                    }
                    for act in std::iter::once(&clause.first).chain(clause.rest.iter().map(|(_, a)| a)) {
                        let guards: Vec<_> = act.mods.iter().filter_map(|m| match m { ir::effect::Modifier::If(c) => Some(c), _ => None }).collect();
                        if !alone && !guards.is_empty() { return unsupported("program: trailing condition on a compound clause not lowered"); }
                        for condition in &guards { out.push(TextStep::If(condition, true)); }
                        out.push(TextStep::Act(clause, act, alone));
                        for _ in guards { out.push(TextStep::EndIf); }
                    }
                    if clause.may && !alone { out.push(TextStep::EndMay); }
                }
            }
            Sentence::Led { lead: Lead::Modifier(ir::effect::Modifier::During(ir::event::During::Step(ir::event::Step::Turn,ir::event::StepOwner::ThatTurns))), rest } => {
                out.push(TextStep::DuringCreatedTurn);walk(rest,out)?;out.push(TextStep::EndDuringCreatedTurn);
            }
            Sentence::Led { lead: Lead::Modifier(ir::effect::Modifier::AfterTurn(turn)), rest } => {
                out.push(TextStep::AfterTurn(*turn));walk(rest,out)?;out.push(TextStep::EndAfterTurn);
            }
            Sentence::Led { lead: Lead::Then, rest } => walk(rest, out)?,
            Sentence::Led { lead: Lead::WhenYouDo, .. } => return unsupported("reflexive: nested scope not lowered"),
            Sentence::Led { lead: Lead::WhenManaSpent { on }, rest } => out.push(TextStep::ManaSpent(on, rest, sentence)),
            Sentence::Led { lead: Lead::Otherwise, rest } => {
                out.push(TextStep::Otherwise); walk(rest, out)?; out.push(TextStep::EndIf);
            }
            Sentence::Led { lead: Lead::Modifier(ir::effect::Modifier::ForEachValue(ir::action::ValueKind::CardType)), rest } => {
                out.push(TextStep::CardTypes(rest));
            }
            Sentence::Led { lead: Lead::Modifier(ir::effect::Modifier::ForEach(Amount::Count(set))), rest }
                if value_iteration::life_or_top(rest).is_some() => {
                out.push(TextStep::CardPayments(set, rest));
            }
            Sentence::Led { lead: Lead::Modifier(ir::effect::Modifier::If(condition)), rest } => {
                out.push(TextStep::If(condition, true));
                walk(rest, out)?;
                out.push(TextStep::EndIf);
            }
            // "For each player, search that player's library …"
            Sentence::Led { lead: Lead::Modifier(ir::effect::Modifier::ForEach(Amount::Count(set))), rest } if set.domain == Domain::Player => {
                out.push(TextStep::Each(set, false));
                walk(rest, out)?;
                out.push(TextStep::EndEach);
            }
            // said of the mana the text adds ([`crate::mana::kept`])
            Sentence::Led { .. } if crate::mana::kept_by(sentence).is_some() => {},
            Sentence::Led { lead: Lead::Modifier(ir::effect::Modifier::Duration(duration)), rest } => {
                out.push(TextStep::Duration(duration));
                walk(rest, out)?;
                out.push(TextStep::EndDuration);
            }
            Sentence::Led { lead: Lead::When { pattern, limit: None | Some(ir::DelayLimit::ThisTurn), next, .. }, rest } => {
                let Sentence::Led { lead: Lead::When { limit, .. }, .. } = sentence else { unreachable!() };
                out.push(TextStep::Delayed(pattern, *limit, *next, rest, sentence));
            }
            // A step marker leads a delayed instruction as well as trailing it
            // (603.7a). Only this sentence belongs to the delayed body; ordinary
            // following sentences still execute during the creating resolution.
            Sentence::Led { lead: Lead::Modifier(ir::effect::Modifier::At {step,whose}), rest } => {
                out.push(TextStep::Later(*step,whose,rest,&[],sentence));
            }
            Sentence::Led { lead, .. } => return unsupported(format!("program: lead not lowered: {}", crate::select::name(lead))),
            Sentence::Static(rule) if matches!(rule.as_ref(), ir::statics::Static::ManaCantBeSpent(ir::mana::SpendOn::Generic)) => out.push(TextStep::ManaGenericForbidden),
            Sentence::Rider(ir::effect::Rider::SpendOnly { .. }) => {},
            Sentence::Rider(ir::effect::Rider::TriggersOnlyOnceEachTurn | ir::effect::Rider::DoThisOnlyOnceEachTurn) => {},
            // "Damage can't be prevented this turn.": a rule the effect puts in force
            Sentence::Static(rule) if matches!(**rule, ir::statics::Static::Unpreventable { .. } | ir::statics::Static::ActivatedAbilities { of: ir::statics::AbilitiesOf::Matching(_),rule: ir::statics::AbilityRule::CantBeActivated { .. },.. }) => out.push(TextStep::Rule(rule)),
            // "If an instant or sorcery spell cast this way would be put into your graveyard,
            // exile it instead.": said of the cast the sentence before offers
            Sentence::Static(rule) if cast_rider(rule).is_some() => out.push(TextStep::CastRider(rule)),
            Sentence::Static(rule) if crate::restrict::Rule::exiled_instead_of_dying(rule).is_some() => out.push(TextStep::ExiledInstead(crate::restrict::Rule::exiled_instead_of_dying(rule).expect("read as one"))),
            Sentence::Static(rule) if matches!(rule.as_ref(), ir::statics::Static::KeywordCost { .. }) => {
                let ir::statics::Static::KeywordCost { keyword, of } = rule.as_ref() else { unreachable!("matched") };
                out.push(TextStep::KeywordCost(*keyword, of));
            }
            _ => return unsupported("program: control flow not lowered"),
        }
        Ok(())
    }
    let mut out = Vec::new();
    for (i, sentence) in effect.0.iter().enumerate() {
        if let Some(change) = crate::announcement::activation_change(sentence, support) { change?; continue; }
        if let Sentence::Restrictions(items) = sentence {
            if !matches!(support.ability(), Some(ir::Ability::Activated { .. })) { return unsupported("activation restriction on non-activated effect"); }
            let _ = items; // compiled into announcement metadata below
            continue; // announcement owns the checked metadata, not body execution
        }
        // 603.12: the reflexive ability includes the instructions following its lead.
        // Announcement restrictions and use limits still belong to the enclosing ability.
        if let Sentence::Led { lead: Lead::WhenYouDo, rest } = sentence {
            out.push(TextStep::Reflexive(rest, &effect.0[i + 1..], None, sentence));
            break;
        }
        // "When you discard a nonland card this way, …": triggered by an event of this
        // resolution (603.12)
        if let Sentence::Led { lead: Lead::When { pattern, limit: Some(ir::DelayLimit::ThisWay), whenever: false, next: false }, rest } = sentence {
            out.push(TextStep::Reflexive(rest, &effect.0[i + 1..], Some(pattern), sentence));
            break;
        }
        // "If you do, at the beginning of your first upkeep, look at the top four cards of your
        // library. You may put one of those cards back on top of your library. Exile the
        // rest.": what an opening-hand action has its player do at that step is all the text
        // that follows (103.6). Elsewhere such a lead is not lowered.
        if let Sentence::Led { lead: Lead::Modifier(ir::effect::Modifier::At { step, whose }), rest } = sentence
            && support.opening()
        {
            out.push(TextStep::Later(*step, whose, rest, &effect.0[i + 1..], sentence));
            break;
        }
        // "At end of combat, exile it …": elsewhere such a lead is said of its own sentence,
        // done by a delayed triggered ability (603.7a)
        if let Sentence::Led { lead: Lead::Modifier(ir::effect::Modifier::AtEndOfCombat), rest } = sentence {
            static NEXT: ir::event::StepOwner = ir::event::StepOwner::The { next: true };
            out.push(TextStep::Later(ir::event::Step::EndOfCombat, &NEXT, rest, &[], sentence));
            continue;
        }
        // "Put five counters on target creature. If this spell was cast from a graveyard,
        // put ten on that creature instead.": the condition is asked once, before either is
        // done (608.2c); the first is done unless it holds, the other if it does.
        if let Some(condition) = effect.0.get(i + 1).and_then(instead) {
            // The instead instruction can reuse the original target ("that creature", "it
            // deals ... instead"), so its declaration is not made conditional. One that
            // declares targets of its own declares alternative targets (601.2c): those of
            // the sentence it replaces are chosen only if the cost or choice was not made,
            // its own only if it was (702.33g, 702.174m), and it can't name the others.
            let alternative = alternative_targets(&effect.0[i + 1]);
            out.extend([TextStep::If(condition, alternative), TextStep::EndIf, TextStep::Otherwise]);
            walk(sentence, &mut out)?;
            out.extend([TextStep::EndIf, TextStep::Instead(alternative)]);
            walk(done_instead(&effect.0[i + 1]), &mut out)?;
            out.push(TextStep::EndIf);
            continue;
        }
        // "Turn target creature face down. It's a 2/2 Cyberman artifact creature.": what the
        // effect lists is the face-down permanent's copiable values (708.2), not an effect
        // applied to it
        if turns_face_down(sentence) && effect.0.get(i + 1).is_some_and(lists_characteristics) {
            return unsupported("turn face down: characteristics the effect lists (708.2)");
        }
        if instead(sentence).is_some() {
            if i == 0 { return unsupported("program: instead, of nothing said before it"); }
            continue; // lowered with the sentence it replaces
        }
        walk(sentence, &mut out)?;
    }
    if out.is_empty() { return unsupported("program: empty body"); }
    Ok(out)
}

/// Every act a sentence says, under whatever leads it.
fn sentence_acts(sentence: &Sentence) -> Vec<&ir::effect::Act> {
    match sentence {
        Sentence::Do(body) => std::iter::once(&body.first).chain(body.rest.iter().map(|(_, c)| c))
            .flat_map(|c| std::iter::once(&c.first).chain(c.rest.iter().map(|(_, a)| a))).collect(),
        Sentence::Led { rest, .. } => sentence_acts(rest),
        _ => Vec::new(),
    }
}
fn turns_face_down(sentence: &Sentence) -> bool {
    sentence_acts(sentence).iter().any(|a| matches!(a.action, Action::TurnFace { up: false, .. }))
}
/// "It's a 2/2 Cyberman artifact creature."
fn lists_characteristics(sentence: &Sentence) -> bool {
    sentence_acts(sentence).iter().any(|a| matches!(a.action, Action::Is(_)))
}

/// A choice of one value: its kind and, of a card name, the characteristics of the card it
/// names (201.4a).
pub(crate) fn scalar_choice(choice: &ir::action::Choice) -> Res<crate::binding::Domain> {
    let ir::action::Choice::Value { kind, count: Amount::Const(1), except, random: false } = choice else {
        return unsupported("choice: unsupported cardinality, randomness or value domain");
    };
    if let ir::action::ValueKind::CardName(set) = kind {
        return Ok(crate::binding::Domain { kind: Kind::CardName, names: crate::binding::Names::compile(set, except)? });
    }
    if !except.is_empty() { return unsupported("choice: restricted value domain"); }
    match kind {
        ir::action::ValueKind::CardType => Ok(Kind::CardType.into()),
        ir::action::ValueKind::CreatureType => Ok(Kind::CreatureType.into()),
        ir::action::ValueKind::Color => Ok(Kind::Color.into()),
        _ => unsupported("choice: unsupported value domain"),
    }
}

/// Is this "It can't be regenerated." / "They can't be regenerated."?
fn bars_regeneration(clause: &Clause, act: &ir::effect::Act) -> bool {
    let of_what_was_named = matches!(&clause.subject, Some(Subject::Selector(s)) if matches!(s.quant, Quant::Definite(Ref::Anaphor(_))));
    let Action::Cant(items) = &act.action else { return false };
    let [item] = items.as_slice() else { return false };
    let regenerated = matches!(&item.event, ir::EventAction::Deed(d) if d.verb == Verb::Regenerate && d.passive && d.args.is_empty());
    of_what_was_named && regenerated && item.within.is_none() && act.mods.is_empty() && clause.rest.is_empty() && !clause.may
}


/// A singular named input whose own completed counter result the condition asks about.
fn countered_subject(condition: &ir::Condition) -> Option<&Selector> {
    let ir::Condition::Happened { pattern, window: Some(ir::condition::Window::ThisWay), times: None, negated: false, .. }=condition else {return None};
    (pattern.subjects.len()==1 && matches!(pattern.subjects[0].quant,
            Quant::Definite(Ref::Anaphor(ir::Anaphor::It | ir::Anaphor::That | ir::Anaphor::The {plural: false})))
        && !pattern.counted() && pattern.during.is_none()
        && pattern.condition.is_none() && pattern.cause.is_none()
        && matches!(pattern.actions.as_slice(),[ir::EventAction::Deed(d)] if d.verb==Verb::Counter && d.passive && d.args.is_empty()))
        .then(|| &pattern.subjects[0])
}

/// "If an ability of an artifact ... is countered this way": its completed
/// counter result supplies the ability, whose exact source is named in this branch.
fn countered_ability_source(condition: &ir::Condition) -> Option<&ObjectSet> {
    let ir::Condition::Happened { pattern, window: Some(ir::condition::Window::ThisWay), times: None, negated: false, .. } = condition else { return None };
    if *pattern != ir::EventPattern::new(pattern.subjects.clone(), pattern.actions.clone()) { return None; }
    let [ability] = pattern.subjects.as_slice() else { return None };
    if ability.quant != Quant::Indefinite(Count::Exactly(Amount::Const(1))) || ability.chooser.is_some() || ability.random.is_some()
        || !matches!(pattern.actions.as_slice(), [ir::EventAction::Deed(d)] if d.verb == Verb::Counter && d.passive && d.args.is_empty()) { return None; }
    let ir::Filter::All(parts) = &ability.set.filter else { return None };
    let [ir::Filter::Atom(ir::Atom::Source(source))] = parts.as_slice() else { return None };
    let mut bare = ability.set.clone(); bare.filter = ir::Filter::any_object();
    let mut plain_source = (**source).clone(); plain_source.filter = ir::Filter::any_object();
    (bare == ObjectSet::of(Domain::Ability) && plain_source == ObjectSet::of(Domain::Permanent)).then_some(source)
}

/// The paired self-replacement, fused only after both references bind to the counter's input.
fn counter_destination_rider<'a>(condition: &ir::Condition, clause: &Clause, act: &'a ir::effect::Act)
    -> Option<(&'a ir::action::Coord<Selector>, action::CounterDestination)> {
    countered_subject(condition)?;
    if !controller_only(clause.subject.as_ref()) || clause.may || clause.also || !clause.rest.is_empty() || act.may { return None; }
    let [ir::effect::Modifier::InsteadOf(ir::statics::Elsewhere::Graveyard(Some(owner)))] = act.mods.as_slice() else { return None };
    // "that player's graveyard" is the owner just named by this destination. Other
    // player references cannot silently become the countered spell's owner.
    let explicit_owner = matches!(owner, Whose::Owner | Whose::Owners);
    let owner = explicit_owner || matches!(owner, Whose::Player(player)
        if player.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::That))
            && player.set == ObjectSet::of(Domain::Player) && player.chooser.is_none() && player.random.is_none());
    if !owner { return None; }
    use action::CounterDestination as Destination;
    match &act.action {
        Action::Exile(objects) if explicit_owner => Some((objects, Destination::Exile)),
        Action::Move { moves, from: None, .. } => match moves.as_slice() {
            [(objects, ir::action::Destination::Hand(Whose::Owner | Whose::Owners))] => Some((objects, Destination::Hand)),
            [(objects, ir::action::Destination::Library(place))] if matches!(place.library, Some(Whose::Owner | Whose::Owners)) && !place.back && place.order.is_none() => {
                let ir::zone_move::Position::One(position) = place.position else { return None };
                Some((objects, Destination::Library(position)))
            }
            _ => None,
        },
        _ => None,
    }
}


struct Emit {
    instrs: Vec<Instr>,
    acts: Vec<ActSpan>,
    next: Slot,
    targets: Vec<TargetSpec>,
    relative_targets: Option<usize>,
    /// The first target group the sentence being lowered declares: of several earlier targets
    /// a target relation names, the one its own sentence declares ("that player").
    sentence_targets: usize,
    target_relations: Vec<Slot>,
    announced_mask: u8,
    alternative_kicker_targets: bool,
    target_x: Option<ir::Amount>,
    // Cached operand and ordinary-reference validity in its original binding context.
    clause_subject: Option<(action::Operand, Res<()>)>,
    /// Capture this clause's quantified subject for the following explicit contrast.
    contrast: Option<ObjectSet>,
    /// The slot a continuous effect fixed the ability's own object in as the subject of its
    /// clause (611.2c). It holds nothing once that object has left; what the same clause has
    /// that object do is still done by it as it last was (608.2h).
    captured_source: Option<Slot>,
    /// The branch past the act of "[do this] unless [a player pays]" (118.12a), taken when
    /// the payment was made; the act's end closes it ([`Emit::end_act`]).
    unless_open: Option<usize>,
    /// The payment of the act being lowered, until its path offers it.
    unless_said: Option<(Selector, Unless)>,
}

impl Emit {
    fn object_choice(&mut self, act: &ir::effect::Act, players: action::Operand, support: &Support<'_>, binder: &mut Binder) -> Res<(choice::Action, Option<Slot>)> {
        let (Action::Discard(objects) | Action::Sacrifice(objects)) = &act.action else { return unsupported("alternative action: only object payments are implemented"); };
        if objects.items.len() > 1 && objects.conj != ir::Conj::And { return unsupported("alternative object payments not lowered"); }
        let mut instructions = Vec::new();
        let mut chosen = None;
        for selector in &objects.items {
            if selector.random.is_some() { return unsupported("choice: at random selection not lowered"); }
            let chooser = match selector.chooser.as_deref() {
                None => choice::Chooser::Actor,
                Some(s) if s.quant == Quant::Definite(Ref::You) => choice::Chooser::Controller,
                Some(s) if matches!(s.quant, Quant::Definite(Ref::Anaphor(_))) => choice::Chooser::Actor,
                _ => return unsupported("choice: chooser not lowered"),
            };
            let instruction = if matches!(act.action, Action::Sacrifice(_)) && selector.quant == Quant::Definite(Ref::This) {
                binder.source_named = Some(binder.at);
                choice::Action::sacrifice_source(players.clone())
            } else if matches!(act.action, Action::Sacrifice(_))
                && (matches!(selector.quant, Quant::Definite(_)) || piles::reference(selector).is_some()) {
                choice::Action::sacrifice_named(players.clone(), vec![self.operand(selector, support, binder)?])
            } else if matches!(act.action, Action::Discard(_)) && matches!(selector.quant, Quant::Definite(_)) {
                choice::Action::discard_named(players.clone(), vec![self.operand(selector, support, binder)?])
            } else if matches!(act.action, Action::Discard(_)) && selector.quant == Quant::The {
                // A whole hand names all its cards (701.9a), not a quantity chosen by its
                // player. Verify whose zone it is before reusing the ordinary discard pool.
                let Some(zone) = selector.set.zone.as_ref() else { return unsupported("discard: a definite group without a hand"); };
                let Some(owner) = zone.owner_of(ir::Zone::Hand).filter(|_| zone.slice.is_none()) else {
                    return unsupported("discard: only the acting player's whole hand");
                };
                if self.operand(owner, support, binder)? != players {
                    return unsupported("discard: the named hand is not the acting player's");
                }
                let mut cards = selector.set.clone(); cards.zone = None;
                choice::Action::discard(players.clone(), binder.selection(&cards, support)?, choice::Count::All)
            } else {
                // "sacrifices a creature … that creature's toughness": what was sacrificed,
                // as it was; one where one player sacrifices one
                if objects.items.len() == 1 && matches!(act.action, Action::Sacrifice(_)) {
                    let one_player = match &players {
                        action::Operand::Controller => true,
                        action::Operand::Reference(slot) => binder.subjects.iter().any(|n| n.slot == *slot && n.bounds.1 <= 1),
                        _ => false,
                    };
                    let max = match &selector.quant {
                        Quant::Indefinite(Count::Exactly(Amount::Const(n)) | Count::UpTo(Amount::Const(n))) if one_player => *n,
                        _ => u32::MAX,
                    };
                    let out = self.slot();
                    binder.name(out, selector.set.clone(), (0, max), false);
                    chosen = Some(out);
                }
                let count = match &selector.quant {
                    Quant::Indefinite(Count::Exactly(n)) => choice::Count::Exactly(quantity(n, support, binder)?),
                    Quant::Indefinite(Count::UpTo(n)) => choice::Count::UpTo(quantity(n, support, binder)?),
                    Quant::Indefinite(Count::AnyNumber) => choice::Count::UpTo(expr::Amount::constant(u32::MAX)),
                    Quant::All | Quant::Each => choice::Count::All,
                    _ => return unsupported("choice: cardinality not lowered"),
                };
                if matches!(act.action, Action::Discard(_)) {
                    choice::Action::discard(players.clone(), binder.selection(&selector.set, support)?, count)
                } else {
                    // Possessive "they" belongs to each instructed player; "you" is still the controller.
                    let mut each = binder.clone();
                    each.clause = Some(expr::Subject::Actor);
                    let set = expr::Set::compile(&selector.set, support, &mut Scoped { binder: &each, support, actor: true, object_actor: false, to_players: false })?;
                    choice::Action::sacrifice(players.clone(), set, count)
                }
            };
            instructions.push(instruction.chosen_by(chooser));
        }
        let instruction = choice::Action::together(instructions)?;
        Ok((instruction, chosen))
    }

    /// "Do this unless that player pays": offer the payment, then do the act only when it
    /// was not made (118.12a); the act's end closes the branch. A player who can't pay
    /// doesn't (118.3).
    fn unless(&mut self, who: &Selector, said: Unless, support: &Support<'_>, binder: &mut Binder) -> Res<()> {
        // "each opponent loses 3 life unless they …": each of them decides for themselves
        let payer = self.operand(who, support, binder).map_err(|e| crate::Unsupported(format!("unless: payer: {}", e.0)))?;
        let bound_single = matches!(&payer, action::Operand::Reference(slot)
            if binder.subjects.iter().any(|n| n.slot == *slot && n.bounds.1 <= 1));
        if !single_actor(Some(&Subject::Selector(who.clone()))) && !bound_single {
            return unsupported("unless: several players need per-player payments");
        }
        let (paid, possible) = (self.slot(), self.slot());
        match said {
            Unless::Cost { said, each } => {
                let cost = crate::cost::Plan::compile(&said, support)?;
                if cost.x() > 0 && !support.x_available() { return unsupported("unless payment has no bound X"); }
                if !cost.phyrexian().is_empty() { return unsupported("unless payment of Phyrexian mana"); }
                let each = each.as_ref().map(|n| quantity(n, support, binder)).transpose()?;
                if each.is_some() { cost.clone().repeated_generic(1)?; }
                self.instrs.push(Instr::Pay { payer, cost, each, paid });
            }
            Unless::Objects(action) => {
                let instruction = match &action {
                    Action::Sacrifice(_) | Action::Discard(_) => {
                        // what the payment chooses is not what the text goes on to name
                        let act = ir::effect::Act { action, mods: Vec::new(), may: false };
                        self.object_choice(&act, payer, support, &mut binder.clone())?.0
                    }
                    Action::Exile(_) | Action::Tap(_) | Action::Move { .. } => {
                        let objects = match &action {
                            Action::Exile(objects) | Action::Tap(objects) => objects,
                            Action::Move { moves, .. } => &moves[0].0,
                            _ => unreachable!(),
                        };
                        let [selector] = objects.items.as_slice() else { return unsupported("unless: a payment of several descriptions"); };
                        if payer != action::Operand::Controller { return unsupported("unless: a payment of objects by another player"); }
                        if selector.chooser.is_some() || selector.random.is_some() { return unsupported("unless: a payment of objects chosen otherwise"); }
                        let Quant::Indefinite(Count::Exactly(count)) = &selector.quant else { return unsupported("unless: a payment of named objects"); };
                        let count = choice::Count::Exactly(quantity(count, support, binder)?);
                        let set = expr::Set::compile(&selector.set, support, &mut binder.scope(support))?;
                        match action {
                            Action::Exile(_) => choice::Action::exile_selected(payer, set, count),
                            Action::Tap(_) => choice::Action::tap_selected(payer, set, count),
                            _ => choice::Action::return_selected(payer, set, count),
                        }
                    }
                    _ => unreachable!("compiled payments of objects"),
                };
                instruction.payment_ok()?;
                self.instrs.push(Instr::Choice { action: instruction, optional: true, payment: true, paid, possible, chosen: None, outcomes: None, actual: None, discarded_before: None });
            }
        }
        self.unless_open = Some(self.instrs.len());
        self.instrs.push(Instr::Branch { condition: condition::Condition::stored(paid, false), end: 0, remember: None, skipped_values: Vec::new() });
        Ok(())
    }

    /// What an outcome asks about ("if you do", "when you do", 118.12, 603.12): the
    /// instruction is done as written or not at all, and records which.
    fn producer(&mut self, instruction: usize) -> Res<()> {
        match &mut self.instrs[instruction] {
            Instr::Choice { action, payment, .. } => { action.payment_ok()?; *payment = true; },
            Instr::Action { action: action::Action::RemoveCounters { .. }, attempt: Some(attempt), .. } => {
                attempt.payment = true;
                attempt.cost = true;
            }
            Instr::Action { attempt: Some(attempt), .. } | Instr::Place { attempt: Some(attempt), .. } => attempt.payment = true,
            _ => {},
        }
        // the objects that action is done to, chosen as it is followed, are chosen
        // wholly or not at all: the outcome asks about the instruction as written
        for at in (0..instruction).rev() {
            match &mut self.instrs[at] {
                Instr::Select { set: SelectionFilter::Action { .. }, pick, .. } => {
                    // choosing none of "up to" pays it (118.12); not modeled here
                    if matches!(pick, Pick::UpTo(_)) { return unsupported("selected object: an up-to choice an outcome reads"); }
                    if let Pick::Exactly(n) = *pick { *pick = Pick::Whole(n); }
                    break;
                }
                Instr::Action { .. } | Instr::Choice { .. } | Instr::Place { .. } | Instr::Branch { .. } | Instr::Select { .. } => break,
                _ => {},
            }
        }
        Ok(())
    }

    fn end_act(&mut self, moves: bool, branch: Option<usize>) {
        if let Some(at) = branch {
            let count = self.instrs.len() - at - 1;
            let Instr::SkipUnless { count: value, .. } = &mut self.instrs[at] else { unreachable!() };
            *value = count;
        }
        if let Some(at) = self.unless_open.take() {
            let end = self.instrs.len();
            let Instr::Branch { end: to, .. } = &mut self.instrs[at] else { unreachable!() };
            *to = end;
        }
        self.acts.push(ActSpan { end: self.instrs.len(), moves, keeps_x: false });
    }
    /// Put "where X is …" where the act that reads it begins: after the nouns that act
    /// states which the number is read from ("on target creature, where X is that creature's
    /// mana value"), and before everything else of it (608.2h). The act has just ended.
    fn define_x(&mut self, amount: expr::Amount, start: usize, gated: bool, binder: &mut Binder) -> Res<()> {
        let reads: Vec<Slot> = amount.scalars().into_iter().filter_map(|s| if let Scalar::Local { slot, .. } = s { Some(slot) } else { None }).collect();
        let names = |i: &Instr| match i {
            Instr::Capture { operand: action::Operand::Targets(_), out } | Instr::Input { out, .. } => Some(*out),
            _ => None,
        };
        let at = self.instrs[start..].iter().rposition(|i| names(i).is_some_and(|out| reads.contains(&out))).map_or(start, |i| start + i + 1);
        if self.instrs[start..at].iter().any(|i| names(i).is_none()) {
            return unsupported("where X: read from what its own instruction does");
        }
        self.instrs.insert(at, Instr::SetX(amount));
        self.acts.last_mut().expect("the act has ended").end += 1;
        // what jumps past the insertion jumps to the same instruction, now one further: "draw
        // a card unless that player pays {X}, where X is …" skips the draw to the act's end
        for i in &mut self.instrs {
            match i {
                Instr::Branch { end, .. } if *end > at => *end += 1,
                Instr::Repeat { from, .. } if *from > at => *from += 1,
                _ => {}
            }
        }
        if gated {
            let Instr::SkipUnless { count, .. } = &mut self.instrs[start - 1] else { unreachable!() };
            *count += 1;
        }
        if let Some((_, _, _, instruction)) = &mut binder.outcome && *instruction >= at { *instruction += 1; }
        Ok(())
    }
    fn slot(&mut self) -> Slot {
        self.next += 1;
        self.next - 1
    }
    /// "From among the cards exiled this way" said after two instructions exiled cards: the
    /// groups of both, as one, each card still where its instruction put it (608.2c). Groups
    /// in different zones, or partly taken already, stay apart and the reference ambiguous.
    fn unite(&mut self, r: &UnresolvedRef, binder: &mut Binder) {
        if r.qualifiers.is_empty() { return; }
        let parts: Vec<usize> = (0..binder.groups.len())
            .filter(|i| r.qualifiers.iter().all(|Qualifier::ThisWay(verb)| binder.groups[*i].by == *verb)).collect();
        let [first, ..] = parts[..] else { return };
        let (by, zone) = (binder.groups[first].by, binder.groups[first].zone);
        if parts.len() < 2 || parts.iter().any(|i| binder.groups[*i].zone != zone || !binder.groups[*i].taken.is_empty()) { return; }
        let members = parts.iter().map(|i| {
            let subjects = self.slot();
            self.instrs.push(Instr::Subjects { cards: binder.groups[*i].slot, out: subjects, still_in: Some(zone) });
            action::Operand::Reference(subjects)
        }).collect();
        let out = self.slot();
        self.instrs.push(Instr::Gather { objects: action::Operand::Union(members), out });
        for i in parts.iter().rev() { binder.groups.remove(*i); }
        binder.groups.push(Group { slot: out, paragraph: binder.paragraph(), by, zone, taken: Vec::new() });
    }
}

impl Emit {
    fn operand(&mut self, s: &Selector, support: &Support<'_>, binder: &mut Binder) -> Res<action::Operand> {
        self.operand_scoped(s, support, binder, false)
    }
    // The dealer exception applies only to this direct anaphor. Recursive operands and
    // expression bindings below use their normal entry points (controllers, filters, amounts).
    fn operand_scoped(&mut self, s: &Selector, support: &Support<'_>, binder: &mut Binder, dealer: bool) -> Res<action::Operand> {
        binder.check_alternative_reference(s)?;
        use action::Operand;
        if s.chooser.is_none() && s.random.is_none() && matches!(s.quant,Quant::All|Quant::Each) && s.set.domain==Domain::Permanent
            && !s.set.other && s.set.zone.is_none() && s.set.controller.is_none() && s.set.owner.is_none() && s.set.caster.is_none()
            && let ir::Filter::All(parts)=&s.set.filter
            && parts.len()==2 && parts.contains(&ir::Filter::Atom(ir::Atom::Type(ir::CardType::Creature)))
            && parts.iter().any(|p|matches!(p,ir::Filter::Atom(ir::Atom::Did{action,when:ir::amount::When::Context,negated:false})
                if matches!(action.as_ref(),ir::event::EventAction::Deed(ir::Deed{verb:Verb::Convoke,passive:false,args})
                    if matches!(args.as_slice(),[(ir::Role::What,ir::Arg::Noun(noun))] if noun.quant==Quant::Definite(Ref::This) && noun.set==ObjectSet::of(Domain::Permanent) && noun.chooser.is_none() && noun.random.is_none()))))
        {return Ok(Operand::Convoked);}

        if let Some((pile, chooser)) = piles::reference(s) {
            let (one, other, zone) = binder.piles.ok_or_else(|| crate::Unsupported("pile reference: no preceding separation".into()))?;
            if !matches!((&s.set.domain, zone), (Domain::Permanent, Zone::Battlefield) | (Domain::Card, Zone::Library | Zone::Graveyard | Zone::Exile)) {
                return unsupported("pile reference: object domain does not match its producer");
            }
            if let Some(chooser) = chooser {
                let requested = self.operand(chooser, support, binder)?;
                let stated = self.instrs.iter().find_map(|i| match i {
                    Instr::Piles { one: produced, chooser, .. } if *produced == one => Some(chooser),
                    _ => None,
                });
                if stated != Some(&requested) { return unsupported("pile reference: chooser differs from its separation"); }
            }
            let cards = if pile == ir::procedure::PileRef::TheOther { other } else { one };
            let out = self.slot();
            self.instrs.push(Instr::Subjects { cards, out, still_in: Some(zone) });
            return Ok(Operand::Reference(out));
        }
        if s.chooser.is_some() || s.random.is_some() { return unsupported("selection: chooser or random selection not lowered"); }
        // "Each of them" and "each of one or two targets" distribute the action over
        // the whole named group. "Targets" without a noun has the damage-target sort (115.4).
        if s.quant == Quant::Each {
            let mut bare = ObjectSet::of(Domain::Object);
            bare.filter = s.set.filter.clone();
            if s.set == bare && let ir::Filter::All(parts) = &s.set.filter
                && let [ir::Filter::Atom(ir::Atom::Among(group))] = parts.as_slice()
                && (matches!(group.quant, Quant::Target(_) | Quant::Definite(_)) || group.set.domain == Domain::AnyTarget)
            { return self.operand(group, support, binder); }
        }
        if s.set.domain == Domain::AnyTarget && let Quant::Indefinite(count) = &s.quant {
            let mut declared = s.clone();
            declared.quant = Quant::Target(count.clone());
            return self.operand(&declared, support, binder);
        }
        // A whole nounless result group ("each of them") can name the actual
        // objects produced by a creation or entry operation. Do not guess between
        // those and a separate looked-at/revealed card group still in scope.
        if let Some(m) = mention(s)
            && m.taking == Taking::All && m.reference == UnresolvedRef::cards(RefForm::Anaphor)
            && m.set == ObjectSet::of(Domain::Card) && binder.groups.is_empty()
        {
            match (&binder.result, binder.entered.as_slice()) {
                (Some((slot, _, _)), []) => return Ok(Operand::Reference(*slot)),
                (None, [cards]) => {
                    let out = self.slot();
                    self.instrs.push(Instr::Subjects { cards: *cards, out, still_in: None });
                    return Ok(Operand::Reference(out));
                }
                (None, []) => {},
                _ => return unsupported("reference: more than one object group could be meant"),
            }
        }
        if s.set.domain == Domain::Card && s.set.zone.is_some()
            && s.quant == Quant::The && s.set.filter == ir::Filter::All(Vec::new())
        {
            let zone = s.set.zone.as_ref().unwrap();
            if let Some(owner) = zone.owner.as_deref().filter(|o| matches!(o.quant, Quant::Target(_))) {
                let Operand::Reference(slot) = self.operand(owner, support, binder)? else { unreachable!("a target is captured") };
                return Ok(Operand::Zone(expr::ZoneCards::with_owner(&s.set, expr::Subject::Local(slot), support, &mut binder.scope(support))?));
            }
            return Ok(Operand::Zone(expr::ZoneCards::compile(&s.set, support, &mut binder.scope(support))?));
        }
        // "all cards from target player's library", "all creature cards in that player's
        // graveyard": one named player's zone, as it is when the instruction is followed
        if s.set.domain == Domain::Card && s.quant == Quant::All
            && let Some(zone) = s.set.zone.as_ref().filter(|z| z.zones.items.len() == 1 && z.slice.is_none())
            && let Some(owner) = zone.owner.as_deref().filter(|o| !crate::select::is_you(o)
                // "their" after "each player" is each of them, which one slot does not hold
                && matches!(o.quant, Quant::Target(Count::Exactly(Amount::Const(1))) | Quant::Definite(Ref::Anaphor(ir::Anaphor::That))))
        {
            let owner = match self.operand(owner, support, binder)? {
                Operand::Reference(slot) => expr::Subject::Local(slot),
                _ => return unsupported("zone: one named owner"),
            };
            return Ok(Operand::Zone(expr::ZoneCards::with_owner(&s.set, owner, support, &mut binder.scope(support))?));
        }
        // Announce one target player nested in a permanent controller relation and retain
        // that exact mention for later "that player". This is not general nested targeting.
        if matches!(s.quant, Quant::All | Quant::Each) && s.set.domain == Domain::Permanent && !s.set.other
            && let Some(relation) = s.set.controller.as_ref().filter(|r| !r.negated && !r.past)
            && relation.who.as_ref() == &Selector::new(Quant::Target(Count::Exactly(Amount::Const(1))), ObjectSet::of(Domain::Player))
        {
            let Operand::Reference(slot) = self.operand(&relation.who, support, binder)? else { unreachable!("target player captured") };
            return Ok(Operand::All(expr::Set::with_controller(&s.set, expr::Subject::Local(slot), support, &mut binder.scope(support))?));
        }
        // A completed-action qualifier is not a literal reference to the
        // earlier target. Keep its successful producer membership (608.2c).
        if let Quant::Definite(Ref::Anaphor(ir::Anaphor::The { plural })) = &s.quant
            && let ir::Filter::All(filters) = &s.set.filter
            && filters.iter().any(|f| matches!(f, ir::Filter::Atom(Atom::Reference(r))
                if r.expected == Kind::Cards && r.form == RefForm::ActionResult && r.qualifiers == [Qualifier::ThisWay(Verb::Exile)])) {
            let expected = ir::Filter::Atom(Atom::Reference(UnresolvedRef { expected: Kind::Cards,
                form: RefForm::ActionResult, qualifiers: vec![Qualifier::ThisWay(Verb::Exile)] }));
            if s.set != (ObjectSet { filter: ir::Filter::All(vec![expected]), ..ObjectSet::of(Domain::Card) }) {
                return unsupported("completed card reference: unsupported result qualifiers");
            }
            let produced = binder.produced.iter().filter(|p| p.verb == Verb::Exile).collect::<Vec<_>>();
            let [produced] = produced.as_slice() else { return unsupported("completed card reference: ambiguous or missing exile producer"); };
            if !plural {
                let singular = self.instrs.iter().any(|i| {
                    let Instr::Action { action: action::Action::Move { groups, .. }, created: Some(out), .. } = i else { return false };
                    let [(objects, _)] = groups.as_slice() else { return false };
                    let [Operand::Reference(original)] = objects.as_slice() else { return false };
                    *out == produced.slot && (binder.subjects.iter().any(|n| n.slot == *original && n.bounds.1 <= 1)
                        || binder.result.as_ref().is_some_and(|(slot, _, max)| slot == original && *max <= 1))
                });
                if !singular { return unsupported("completed card reference: singular noun without a singular producer"); }
            }
            let out = self.slot();
            self.instrs.push(Instr::Subjects { cards: produced.slot, out, still_in: None });
            return Ok(Operand::Within { of: Box::new(Operand::Reference(out)), place: action::Place::Shared(Zone::Exile) });
        }
        match &s.quant {
            Quant::Definite(Ref::AttachedTo(_)) => {
                // An explicitly named enchanted/equipped object is the antecedent of
                // the following "it". Capture the incarnation now; do not substitute
                // the Aura/Equipment source or follow a later zone-change successor.
                let out = self.slot();
                self.instrs.push(Instr::Capture { operand: Operand::Subject(expr::Subject::Attached), out });
                // A repeated direct mention names the same relation afresh; it
                // supersedes the previous attached-object antecedent, rather than
                // making a later pronoun ambiguous between identical mentions.
                binder.subjects.retain(|named| !named.attached);
                binder.name(out, s.set.clone(), (0, 1), false);
                binder.subjects.last_mut().expect("captured attachment").attached = true;
                // Damage keeps its dynamic projection for announcement exclusions
                // and LKI; the separate capture still supplies subsequent pronouns.
                Ok(if dealer { Operand::Subject(expr::Subject::Attached) } else { Operand::Reference(out) })
            },
            // "the exiled cards": what this text exiled, all of it, as it is in exile; else
            // what an ability linked to this one exiled (607.2a)
            Quant::Definite(Ref::Anaphor(ir::Anaphor::TheParticiple { what: Verb::Exile, plural })) => {
                let operand = match binder.exiled_now.as_slice() {
                    [] => Operand::Subject(expr::Subject::LinkedExile(support.exile_producer()?)),
                    [one] => Operand::Reference(*one),
                    several => Operand::Union(several.iter().map(|slot| Operand::Reference(*slot)).collect()),
                };
                // A possessive may introduce this card before the next act names it
                // directly: "the exiled card's owner may cast that card" (608.2c).
                let out = self.slot();
                self.instrs.push(Instr::Capture { operand, out });
                binder.name(out, s.set.clone(), (0, if *plural { u32::MAX } else { 1 }), false);
                Ok(Operand::Reference(out))
            },
            Quant::Definite(Ref::You) => Ok(Operand::Controller),
            Quant::Definite(Ref::This) => { binder.source_named = Some(binder.at); Ok(Operand::Source) }
            // "that card from your graveyard": the card named, only while it is there
            Quant::Definite(Ref::Anaphor(_)) if s.set.domain == Domain::Card && s.set.zone.is_some() => {
                let zone = s.set.zone.as_ref().expect("a zone is said");
                let place = match zone.zones.items.as_slice() {
                    [ir::Zone::Exile] if zone.owner.is_none() && zone.slice.is_none() => action::Place::Shared(Zone::Exile),
                    _ => action::Place::Pile(expr::ZoneCards::compile(&s.set, support, &mut binder.scope(support))?),
                };
                let unzoned = Selector { set: ObjectSet { zone: None, ..s.set.clone() }, ..s.clone() };
                Ok(Operand::Within { of: Box::new(self.operand(&unzoned, support, binder)?), place })
            }
            Quant::Definite(Ref::Anaphor(_)) => {
                let referent = binder.referent_scoped(s, support, true, false, dealer)?;
                self.reference_operand(s, referent, support, binder)
            },
            Quant::Definite(Ref::ControllerOf(of)) => Ok(Operand::ControllerOf(Box::new(self.possessor(of, support, binder)?))),
            // said by a permanent's ability, of the creature it is on (508.5)
            Quant::Definite(Ref::DefendingPlayer) if !support.spell_text() => Ok(Operand::DefendingPlayer),
            Quant::Definite(Ref::OwnerOf(of)) => Ok(Operand::OwnerOf(Box::new(self.possessor(of, support, binder)?))),
            Quant::Target(_) => {
                if self.alternative_kicker_targets { return unsupported("kicker instead: alternative target declarations are not implemented"); }
                if binder.each.is_some() { return unsupported("for each player: per-player target announcements are not implemented"); }
                let n = self.targets.len();
                let mut selector = s.clone();
                let mut distinct = Vec::new();
                if selector.set.other && n > 0 {
                    let Some(base) = self.relative_targets.filter(|base| *base < n) else {
                        return unsupported("another target following a separate action: its referent is not modeled");
                    };
                    selector.set.other = false;
                    distinct.extend(base..n);
                }
                // Attached is announcement-readable; frame-local dealer slots are not.
                // Existing explicit comparisons and prior-target distinctness keep precedence.
                let attached = binder.recipients.subject == Some(expr::Subject::Attached);
                let mut spec = TargetSpec::compile_after_with_attached_dealer(selector, support, self.target_x.as_ref(), &self.targets, attached, self.sentence_targets)?;
                // A public-zone “other card” after a named occurrence participant
                // contrasts with that participant, not the ability source. Only a
                // retained input is available before resolution/target announcement.
                if s.set.other && s.set.domain == Domain::Card
                    && s.set.zone.as_ref().is_some_and(|z| z.zones.items.len() == 1
                        && matches!(z.zones.items[0], ir::Zone::Graveyard | ir::Zone::Exile))
                    && let Some(named) = binder.subjects.iter().rev().find(|n| !n.target && !n.own && kinds(&n.set) == (false, true))
                    && let Some(input) = self.instrs.iter().rev().find_map(|i| match i {
                        Instr::Input { input, out } if *out == named.slot => Some(*input),
                        _ => None,
                    })
                {
                    spec.set = spec.set.excluding(expr::Subject::MovedInput(input));
                }
                spec.distinct_from = distinct;
                spec.announced_mask = self.announced_mask;
                self.targets.push(spec);
                let out = self.slot();
                self.instrs.push(Instr::Capture { operand: Operand::Targets(n), out });
                binder.name(out, s.set.clone(), self.targets[n].bounds, false);
                binder.subjects.last_mut().expect("target just named").target = true;
                // A singular player named by the target's controller restriction is also
                // an antecedent. Capture before the action: destroying the target must not
                // turn "that player" into the card's owner in its new zone (608.2h).
                if self.targets[n].bounds.1 == 1 {
                    if let Some(relation) = &s.set.controller
                        && !relation.negated && !relation.past
                        && matches!(relation.who.quant, Quant::Indefinite(Count::Exactly(Amount::Const(1))))
                        && relation.who.set.domain == Domain::Player
                    {
                        let player = self.slot();
                        self.instrs.push(Instr::Capture { operand: Operand::ControllerOf(Box::new(Operand::Reference(out))), out: player });
                        binder.name(player, relation.who.set.clone(), (0, 1), false);
                        binder.subjects.last_mut().expect("controller just named").controlled_target = Some(out);
                        self.target_relations.push(player);
                    } else if s.set.domain == Domain::Card
                        && let Some(who) = s.set.zone.as_ref().and_then(|z| z.owner.as_deref())
                        && matches!(who.quant, Quant::Indefinite(Count::Exactly(Amount::Const(1))))
                        && who.set.domain == Domain::Player && who.chooser.is_none() && who.random.is_none()
                    {
                        // "exile target card from an opponent's graveyard. … that player loses
                        // 1 life": the owner of the graveyard it is in is named too, captured
                        // before it moves, though moving keeps its owner (400.3, 608.2c)
                        let player = self.slot();
                        self.instrs.push(Instr::Capture { operand: Operand::OwnerOf(Box::new(Operand::Reference(out))), out: player });
                        binder.name(player, who.set.clone(), (0, 1), false);
                        binder.subjects.last_mut().expect("owner just named").controlled_target = Some(out);
                        self.target_relations.push(player);
                    }
                }
                Ok(Operand::Reference(out))
            }
            Quant::Indefinite(Count::Exactly(Amount::Const(n)) | Count::UpTo(Amount::Const(n)))
                if s.set.domain == Domain::Card && s.set.zone.as_ref().is_some_and(|zone|
                    zone.owner_of(ir::Zone::Hand).is_some_and(crate::select::is_you)) => {
                // Choosing a card from your hand is private. Reuse the retained selection
                // instruction; only a later reveal or zone move makes it public (402.3).
                let set = expr::Set::compile(&s.set, support, &mut binder.scope(support))?;
                let (from, selected, subjects) = (self.slot(), self.slot(), self.slot());
                self.instrs.push(Instr::Gather { objects: Operand::All(set), out: from });
                let pick = if matches!(s.quant, Quant::Indefinite(Count::UpTo(_))) { Pick::UpTo(*n as usize) } else { Pick::Exactly(*n as usize) };
                self.instrs.push(Instr::Select { from,
                    set: expr::Cards::compile(&ObjectSet::of(Domain::Card), support, &mut binder.scope(support))?.into(),
                    pick, chooser: Who::Controller, gate: None, still_in: Zone::Hand, out: selected });
                self.instrs.push(Instr::Subjects { cards: selected, out: subjects, still_in: None });
                binder.name(subjects, s.set.clone(), (0,*n), false);
                Ok(Operand::Reference(subjects))
            }
            Quant::All | Quant::Each => Ok(Operand::All(expr::Set::compile_excluding(
                &s.set, support, &mut binder.scope(support), || binder.root_other(&s.set, support),
                binder.recipients.distributive,
            )?)),
            _ => unsupported(format!("selection not lowered: {:?}", s.quant)),
        }
    }
    /// A singular explicit choice retains its own card slot and source-zone proof.
    /// Ordinary targets and arbitrary earlier subjects cannot supply this partition input.
    fn retained_selection(&mut self, selector: &Selector, support: &Support<'_>, binder: &mut Binder) -> Res<Option<(Slot, Zone)>> {
        if !matches!(selector.quant, Quant::Definite(Ref::Anaphor(ir::Anaphor::That | ir::Anaphor::It)))
            || selector.set != ObjectSet::of(Domain::Card) || selector.chooser.is_some() || selector.random.is_some() { return Ok(None); }
        let Some((subjects, _, max)) = &binder.result else { return Ok(None) };
        let subjects = *subjects;
        if *max > 1 || !binder.subjects.iter().any(|n| n.slot == subjects && n.explicit_choice) { return Ok(None); }
        if self.operand(selector, support, binder)? != action::Operand::Reference(subjects) { return Ok(None); }
        let Some(cards) = self.instrs.iter().find_map(|i| match i {
            Instr::Subjects { cards, out, .. } if *out == subjects => Some(*cards), _ => None,
        }) else { return Ok(None) };
        let zones: Vec<_> = binder.groups.iter().filter(|g| g.taken.contains(&cards)).map(|g| g.zone).collect();
        let [zone] = zones.as_slice() else { return unsupported("selected movement: no unique retained group source"); };
        Ok(Some((cards, *zone)))
    }

    /// Origin contract of the exact card producer, independent of casting cost (608.2g).
    /// Card copies inherit their originals' zone (707.12); spell copies are not cast.
    fn cast_origin(&self, operand: &action::Operand) -> Option<CastOrigin> {
        use action::Operand;
        // A later anaphor may still name the move's input (including a whole
        // multi-player search result). Authorize only the exact successful public
        // successors, or its unchanged original if the move failed (609.3).
        // A later hidden/unsupported move must not reuse older proof.
        if let Operand::Reference(original) = operand
            && let Some(origin) = self.instrs.iter().rev().find_map(|instruction| match instruction {
                Instr::MoveTogether { parts,entered } if parts.iter().any(|(src,_)|src==original) =>
                    Some(entered.iter().find(|(src,_)|src==original).map(|(_,out)|CastOrigin::Retained {original:*original,moved:*out})),
                Instr::Action { action: action::Action::Move { groups, .. }, .. }
                    if groups.iter().any(|(objects, _)| objects.contains(operand)) =>
                        Some(instruction.cast_result().map(|(out, _)| CastOrigin::Retained { original: *original, moved: out })),
                _ => None,
            }) {
            return origin;
        }
        match operand {
            Operand::Zone(cards) => Some(CastOrigin::Fixed(cards.zone())),
            Operand::Within { of, .. } if matches!(self.cast_origin(of), Some(CastOrigin::Produced(_))) => self.cast_origin(of),
            Operand::Within { place, .. } => Some(CastOrigin::Fixed(match place {
                action::Place::Pile(cards) => cards.zone(),
                action::Place::Shared(zone) => *zone,
            })),
            Operand::Subject(expr::Subject::LinkedExile(_)) => Some(CastOrigin::Fixed(Zone::Exile)),
            Operand::Union(parts) => {
                let mut zones = parts.iter().map(|part| self.cast_origin(part));
                let first = zones.next()??;
                zones.all(|zone| zone == Some(first)).then_some(first)
            }
            Operand::Reference(slot) => {
                self.instrs.iter().rev().find_map(|instruction| match instruction {
                    Instr::Capture { operand, out } | Instr::Gather { objects: operand, out }
                        if out == slot => Some(self.cast_origin(operand)),
                    // Creation has its own exact identities, even in a hidden zone
                    // (707.12), independently of movement tracking of its original.
                    Instr::CopyCards { out, .. } if out == slot => Some(Some(CastOrigin::Produced(*out))),
                    Instr::Subjects { cards, out, still_in } if out == slot =>
                        Some(still_in.map(CastOrigin::Fixed).or_else(|| self.cast_origin(&Operand::Reference(*cards)))),
                    Instr::Show { from, out, .. } if out == slot => Some(self.cast_origin(&Operand::Reference(*from))),
                    Instr::Select { out, still_in, .. } | Instr::Remainder { out, still_in, .. } if out == slot => Some(Some(CastOrigin::Fixed(*still_in))),
                    Instr::TakeTop { out, .. } if out == slot => Some(Some(CastOrigin::Fixed(Zone::Library))),
                    Instr::TakeUntil { out, matched, .. } if out == slot || matched == slot => Some(Some(CastOrigin::Fixed(Zone::Library))),
                    Instr::LibraryScan { all, hit, .. } if all == slot || hit == slot => Some(Some(CastOrigin::Fixed(Zone::Exile))),
                    Instr::Exile { out, .. } if out == slot => Some(Some(CastOrigin::Produced(*out))),
                    Instr::Action { created: Some(out), .. } if out == slot =>
                        Some(instruction.cast_result().map(|(out, _)| CastOrigin::Produced(out))),
                    Instr::MoveTogether { entered, .. } if entered.iter().any(|(_,out)|out==slot) =>
                        Some(Some(CastOrigin::Produced(*slot))),
                    _ => None,
                }).flatten()
            }
            _ => None,
        }
    }
    fn reference_operand(&mut self, s: &Selector, referent: Referent, support: &Support<'_>, binder: &mut Binder) -> Res<action::Operand> {
        use action::Operand;
        let operand: Operand = match referent {
            Referent::Local(slots) => match slots.as_slice() {
                [slot] => Ok(Operand::Reference(binder.object(*slot))),
                _ => Ok(Operand::Union(slots.into_iter().map(|slot| Operand::Reference(binder.object(slot))).collect())),
            },
            Referent::Source => Ok(Operand::Source),
            Referent::Input(input) => {
                let out = self.slot();
                self.instrs.push(Instr::Input { input, out });
                let (players, max) = input::signature(support, input)?;
                let description = if players && input::bare_personal(s) { ObjectSet::of(Domain::Player) } else { s.set.clone() };
                binder.name(out, description, (0, max), input::own(support, input));
                Ok(Operand::Reference(out))
            }
        }?;
        // "Those/that creatures you don't control" restricts the retained
        // participant(s) as this action is followed (608.2h). It does not identify
        // a new participant or retarget the original announced group.
        if matches!(s.quant,Quant::Definite(Ref::Anaphor(form)) if !matches!(form,ir::Anaphor::The {plural:false}))
            && let Some(relation)=&s.set.controller
        {
            return Ok(Operand::Matching(action::Matching {of:Box::new(operand),
                filter:Box::new(expr::Filter::compile(&s.set.filter,support,&mut binder.scope(support))?),
                controller:Some(expr::Controller::compile(relation)?)}));
        }
        Ok(operand)
    }
    /// The one target group an amount is divided or distributed among (601.2d), and that
    /// amount, which is known as the spell or ability is put on the stack: a number, the X
    /// announced, or what the text says X is, counted then (603.3d).
    fn divided_group(&mut self, among: &[action::Operand], amount: &Amount, support: &Support<'_>, binder: &Binder) -> Res<usize> {
        // a target group is captured where the text names it
        let group = match among {
            [action::Operand::Targets(group)] => Some(*group),
            [action::Operand::Reference(slot)] => self.instrs.iter().rev().find_map(|i| match i {
                Instr::Capture { operand: action::Operand::Targets(group), out } if out == slot => Some(*group),
                _ => None,
            }),
            _ => None,
        };
        let Some(ref group) = group else { return unsupported("division among other than one target group (601.2d)"); };
        let total = match (amount, &self.target_x) {
            (Amount::X, Some(stated)) => expr::Amount::compile(stated, support, &mut expr::linked(support))?,
            (Amount::Const(_) | Amount::X, _) => quantity(amount, support, binder)?,
            _ => return unsupported("division of an amount not known as it is announced (601.2d)"),
        };
        let spec = self.targets.get_mut(*group).expect("an operand names a target group of its program");
        if spec.divided.replace(total).is_some() { return unsupported("two divisions among one target group (601.2d)"); }
        Ok(*group)
    }
    /// Bind a damage recipient once, carrying the actual producer's player possibility
    /// alongside the operand. A union target stays a union even when this cast picks an object.
    fn damage_recipients(&mut self, objects: &ir::Objects, support: &Support<'_>, binder: &mut Binder) -> Res<(Vec<action::Operand>, bool)> {
        if objects.conj != ir::Conj::And && objects.items.len() > 1 {
            return unsupported("choice between object groups not lowered");
        }
        let mut players = false;
        let mut operands = Vec::new();
        for s in &objects.items {
            let bare_it = s.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::It))
                && s.set == ObjectSet::of(Domain::Object) && s.chooser.is_none() && s.random.is_none();
            if bare_it || input::bare_personal(s) {
                let referent = binder.referent_scoped(s, support, true, true, false)?;
                players |= match &referent {
                    Referent::Local(slots) => slots.iter().any(|slot| {
                        binder.subjects.iter().find(|n| n.slot == *slot).is_some_and(|n| kinds(&n.set).0)
                            || binder.result.as_ref().is_some_and(|(out, set, _)| out == slot && kinds(set).0)
                    }),
                    Referent::Source => false,
                    Referent::Input(input) => input::signature(support, *input)?.0,
                };
                operands.push(self.reference_operand(s, referent, support, binder)?);
            } else {
                players |= kinds(&s.set).0;
                operands.push(self.object(s, support, binder)?);
            }
        }
        Ok((operands, players))
    }
    /// Whose controller or owner is named: the clause's subject where "its" can be its.
    fn possessor(&mut self, of: &Selector, support: &Support<'_>, binder: &mut Binder) -> Res<action::Operand> {
        match binder.possessor(of) {
            Some(expr::Subject::Local(slot)) => Ok(action::Operand::Reference(slot)),
            Some(expr::Subject::Source) => Ok(action::Operand::Source),
            Some(expr::Subject::Attached) => Ok(action::Operand::Subject(expr::Subject::Attached)),
            _ => self.operand(of, support, binder),
        }
    }
    /// A resolution selection retains its answer before any following instruction runs.
    fn select_cards(&mut self, from: Slot, set: expr::Cards, description: ObjectSet,
        pick: Pick, zone: Zone, chooser: Who, binder: &mut Binder) -> (action::Operand, Slot) {
        let (cards, subjects) = (self.slot(), self.slot());
        let max = match &pick { Pick::Exactly(n) | Pick::UpTo(n) | Pick::Whole(n) => *n as u32, _ => u32::MAX };
        self.instrs.push(Instr::Select { from, set: set.into(), pick, chooser, gate: None, still_in: zone, out: cards });
        self.instrs.push(Instr::Subjects { cards, out: subjects, still_in: None });
        binder.result = Some((subjects, description.clone(), max));
        binder.name(subjects, description, (0, max), false);
        (action::Operand::Reference(subjects), cards)
    }
    /// The one player a manifest, a cloak or a manifest dread is said of: the player who
    /// looks, chooses, and controls the face-down permanent (701.40a, 701.58a, 701.62a).
    fn face_down_actor(&mut self, clause: &Clause, support: &Support<'_>, binder: &mut Binder) -> Res<Who> {
        if !single_actor(clause.subject.as_ref()) { return unsupported("manifest or cloak: one instructed player"); }
        Ok(match self.clause_subject(clause, support, binder)? {
            action::Operand::Controller => Who::Controller,
            action::Operand::Reference(slot) => Who::Of(slot),
            actor => {
                let slot = self.slot();
                self.instrs.push(Instr::Capture { operand: actor, out: slot });
                binder.name(slot, ObjectSet::of(Domain::Player), (0, 1), false);
                binder.clause = Some(expr::Subject::Local(slot));
                Who::Of(slot)
            }
        })
    }
    /// Put the card (a group of at most one, `card`) onto the battlefield face down under
    /// `by`'s control (701.40a, 701.58a). Later text naming the card ("attach this Equipment to
    /// it") names the face-down permanent (400.7j). Returns the permanent's slot.
    fn enter_face_down(&mut self, card: Slot, by: Who, concealment: crate::face::Concealment, binder: &mut Binder) -> Slot {
        let (entered, permanent) = (self.slot(), self.slot());
        binder.entered.push(entered);
        self.instrs.push(Instr::Action { action: action::Action::Move { groups: vec![(vec![action::Operand::Reference(card)], action::Destination::Battlefield {
            owner_controls: false, by, tapped: false, counters: Vec::new(), attacking: None, transformed: false, face_down: Some(concealment),
        })], said: ir::action::MoveVerb::Put }, created: Some(entered), attempt: None });
        self.instrs.push(Instr::Subjects { cards: entered, out: permanent, still_in: Some(Zone::Battlefield) });
        for named in &mut binder.subjects {
            if named.slot == card || named.object == Some(card) { named.object = Some(permanent); }
        }
        permanent
    }
    /// A bounded non-targeted permanent choice, retained before the ordinary action.
    fn select_permanents(&mut self, selector: &Selector, chooser: Who, verb: Verb,
        support: &Support<'_>, binder: &mut Binder) -> Res<action::Operand> {
        if selector.set.domain != Domain::Permanent || selector.set.zone.is_some() {
            return unsupported("selected permanent action: battlefield objects and one retained chooser");
        }
        self.select_chosen(selector, chooser, verb, ir::Role::What, false, support, binder)
    }
    /// "Put a +1/+1 counter on a creature you control", "return a creature card from your
    /// graveyard to your hand": not a target but an object chosen as the instruction is
    /// followed, by the player who follows it, among those the action can be done to
    /// (608.2d). A permanent, or a card in a graveyard, still there when it is chosen.
    fn select_chosen(&mut self, selector: &Selector, chooser: Who, verb: Verb, role: ir::Role, enters: bool,
        support: &Support<'_>, binder: &mut Binder) -> Res<action::Operand> {
        let permanent = selector.set.domain == Domain::Permanent && selector.set.zone.is_none();
        let Some(still_in) = (if permanent { Some(Zone::Battlefield) } else { chosen_on_resolution(&selector.set) }) else {
            return unsupported("selected object: a permanent or a card in a graveyard");
        };
        if selector.chooser.is_some() || selector.random.is_some() {
            return unsupported("selected permanent action: battlefield objects and one retained chooser");
        }
        let (pick, max) = match selector.quant {
            // "zero cards" is a cost paid by doing nothing (118.3), not a choice
            Quant::Indefinite(Count::Exactly(Amount::Const(n))) if n > 0 => (Pick::Exactly(n as usize), n),
            Quant::Indefinite(Count::UpTo(Amount::Const(n))) => (Pick::UpTo(n as usize), n),
            Quant::Indefinite(Count::AnyNumber) => (Pick::AnyNumber, u32::MAX),
            _ => return unsupported("selected permanent action: fixed bounded count required"),
        };
        let set = expr::Set::compile(&selector.set, support, &mut binder.scope(support))?;
        let (from, selected, subjects) = (self.slot(), self.slot(), self.slot());
        self.instrs.push(Instr::Gather { objects: action::Operand::All(set.clone()), out: from });
        self.instrs.push(Instr::Select { from, set: SelectionFilter::Action { set, verb, role, enters },
            pick, chooser, gate: None, still_in, out: selected });
        self.instrs.push(Instr::Subjects { cards: selected, out: subjects, still_in: None });
        binder.result = Some((subjects, selector.set.clone(), max));
        binder.name(subjects, selector.set.clone(), (0, max), false);
        Ok(action::Operand::Reference(subjects))
    }
    /// "A token that's a copy of a creature token you control": the original is chosen as the
    /// token is created, by the player who creates it, among the permanents that can be copied
    /// (608.2d). With none, no token is created (701.36b).
    fn chosen_copy_source(&mut self, of: &Selector, clause: &Clause, optional: bool, support: &Support<'_>, binder: &mut Binder) -> Res<action::Operand> {
        if optional || binder.each.is_some() || binder.actor.is_some() || !controller_only(clause.subject.as_ref()) {
            return unsupported("chosen copy original: one mandatory choice by the controller");
        }
        self.select_chosen(of, Who::Controller, Verb::CreateCopy, ir::Role::Of, false, support, binder)
    }
    /// What an action is done to, an indefinite object of it ("a creature card from your
    /// graveyard") chosen as the action is followed (608.2d) by the player who follows it:
    /// the controller, or the one player the clause is said of ("Target opponent exiles a
    /// creature they control."). A choice per player, or as part of an optional instruction,
    /// is not lowered here.
    fn chosen_objects(&mut self, objects: &ir::Objects, clause: &Clause, optional: bool, verb: Verb, role: ir::Role, enters: bool,
        support: &Support<'_>, binder: &mut Binder) -> Res<Vec<action::Operand>> {
        // "Target opponent exiles a card from their hand": a card of a hidden zone, chosen by
        // the one player whose hand it is (608.2d), unseen by the others (402.3)
        let from_their_hand = |s: &Selector| matches!(s.quant, Quant::Indefinite(Count::Exactly(Amount::Const(1..)) | Count::UpTo(Amount::Const(_))))
            && s.set.domain == Domain::Card && s.chooser.is_none() && s.random.is_none() && !s.set.other
            && s.set.zone.as_ref().is_some_and(|zone| zone.slice.is_none() && zone.zones.items.as_slice() == [ir::Zone::Hand]
                && zone.owner.as_deref().is_some_and(|o| o.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::They))))
            && !controller_only(clause.subject.as_ref());
        let chosen = |s: &Selector| matches!(s.quant, Quant::Indefinite(_)) && chosen_on_resolution(&s.set).is_some() || from_their_hand(s);
        if !objects.items.iter().any(chosen) { return self.objects(objects, support, binder); }
        // "a creature they control and their graveyard": the one chosen, and the others named
        // beside it, all done together
        if objects.items.iter().filter(|s| chosen(s)).count() > 1 || objects.conj != ir::Conj::And && objects.items.len() > 1 {
            return unsupported("selected object: one description");
        }
        if optional || binder.each.is_some() {
            return unsupported("selected object: one mandatory choice by the controller");
        }
        let chooser = if binder.actor.is_none() && controller_only(clause.subject.as_ref()) { Who::Controller } else {
            if !single_actor(clause.subject.as_ref()) { return unsupported("selected object: a choice by several players"); }
            self.actor(clause, support, binder)?
        };
        let operands: Vec<action::Operand> = objects.items.iter().map(|s| if from_their_hand(s) { self.chosen_from_hand(s, chooser, support, binder) }
            else if chosen(s) { self.select_chosen(s, chooser, verb, role, enters, support, binder) } else { self.object(s, support, binder) }).collect::<Res<_>>()?;
        if objects.items.len() == 1 { return Ok(operands); }
        // "exile it and up to one creature …, then return those cards": what is said of them
        // after is said of every group together, not of the one chosen alone
        let all = self.slot();
        self.instrs.push(Instr::Capture { operand: action::Operand::Union(operands.clone()), out: all });
        let set = ObjectSet::of(Domain::Object);
        binder.name(all, set.clone(), (0, u32::MAX), false);
        binder.result = Some((all, set, u32::MAX));
        Ok(operands)
    }
    /// A card of a hand, chosen by this player as the instruction is followed (608.2d): one
    /// still in that hand. The choice is private; only what is done with it after may make it
    /// public (402.3).
    fn chosen_from_hand(&mut self, s: &Selector, chooser: Who, support: &Support<'_>, binder: &mut Binder) -> Res<action::Operand> {
        let (pick, max) = match s.quant {
            Quant::Indefinite(Count::Exactly(Amount::Const(n))) => (Pick::Exactly(n as usize), n),
            Quant::Indefinite(Count::UpTo(Amount::Const(n))) => (Pick::UpTo(n as usize), n),
            _ => unreachable!("a fixed number of cards"),
        };
        let cards = expr::ZoneCards::compile(&s.set, support, &mut binder.scope(support))?;
        let (from, selected, subjects) = (self.slot(), self.slot(), self.slot());
        self.instrs.push(Instr::Gather { objects: action::Operand::Zone(cards), out: from });
        let description = ObjectSet { zone: None, ..s.set.clone() };
        let set = binder.selection(&description, support)?;
        self.instrs.push(Instr::Select { from, set: set.into(), pick, chooser, gate: None, still_in: Zone::Hand, out: selected });
        self.instrs.push(Instr::Subjects { cards: selected, out: subjects, still_in: Some(Zone::Hand) });
        binder.result = Some((subjects, s.set.clone(), max));
        binder.name(subjects, s.set.clone(), (0, max), false);
        Ok(action::Operand::Reference(subjects))
    }
    /// The one player a clause is said of, who does what it says and makes its choices
    /// (608.2d), named for "they" and "that player" in what follows.
    fn actor(&mut self, clause: &Clause, support: &Support<'_>, binder: &mut Binder) -> Res<Who> {
        Ok(match self.clause_subject(clause, support, binder)? {
            action::Operand::Controller => Who::Controller,
            action::Operand::Reference(slot) => Who::Of(slot),
            actor => {
                let slot = self.slot();
                self.instrs.push(Instr::Capture { operand: actor, out: slot });
                binder.name(slot, ObjectSet::of(Domain::Player), (0, 1), false);
                binder.clause = Some(expr::Subject::Local(slot));
                Who::Of(slot)
            }
        })
    }
    fn objects(&mut self, objects: &ir::Objects, support: &Support<'_>, binder: &mut Binder) -> Res<Vec<action::Operand>> {
        if objects.conj != ir::Conj::And && objects.items.len() > 1 {
            return unsupported("choice between object groups not lowered");
        }
        let operands: Vec<_> = objects.items.iter().map(|s| self.object(s, support, binder)).collect::<Res<_>>()?;
        let slots: Vec<Slot> = operands.iter().filter_map(|o| match o { action::Operand::Reference(slot) => Some(*slot), _ => None }).collect();
        if slots.len() > 1 { binder.coordinated.push(slots); }
        Ok(operands)
    }
    /// What an action is done to. A described set of objects, and whoever controls or owns
    /// something, is fixed as the action begins and kept for what the text says of it
    /// afterwards: "Put a +1/+1 counter on each creature you control. Untap those creatures.",
    /// "deals damage to that land's controller equal to … that player's graveyard".
    fn object(&mut self, s: &Selector, support: &Support<'_>, binder: &mut Binder) -> Res<action::Operand> {
        let operand = self.operand(s, support, binder)?;
        let (set, max) = match (&s.quant, &operand) {
            (Quant::All | Quant::Each, action::Operand::All(_)) => (s.set.clone(), u32::MAX),
            (Quant::Definite(Ref::ControllerOf(_) | Ref::OwnerOf(_)), action::Operand::ControllerOf(of) | action::Operand::OwnerOf(of)) => {
                let one = match &**of {
                    action::Operand::Source => true,
                    action::Operand::Reference(slot) => binder.subjects.iter().any(|n| n.slot == *slot && n.bounds.1 <= 1),
                    _ => false,
                };
                (ObjectSet::of(Domain::Player), if one { 1 } else { u32::MAX })
            }
            _ => return Ok(operand),
        };
        Ok(self.keep(operand, set, max, binder))
    }
    fn keep(&mut self, operand: action::Operand, set: ObjectSet, max: u32, binder: &mut Binder) -> action::Operand {
        let out = self.slot();
        self.instrs.push(Instr::Capture { operand, out });
        binder.name(out, set, (0, max), false);
        action::Operand::Reference(out)
    }
    fn subject(&mut self, subject: Option<&Subject>, support: &Support<'_>, binder: &mut Binder) -> Res<action::Operand> {
        self.subject_scoped(subject, support, binder, false)
    }
    fn subject_scoped(&mut self, subject: Option<&Subject>, support: &Support<'_>, binder: &mut Binder, dealer: bool) -> Res<action::Operand> {
        match subject {
            None => Ok(action::Operand::Controller),
            Some(Subject::Selector(s) | Subject::Each(s)) => self.operand_scoped(s, support, binder, dealer),
            Some(Subject::Set(set)) => Ok(action::Operand::All(expr::Set::compile_root(set, support, &mut binder.scope(support))?)),
            Some(Subject::And { items, .. }) => Ok(action::Operand::Union(items.iter().map(|s| self.subject_scoped(Some(s), support, binder, dealer)).collect::<Res<_>>()?)),
            _ => unsupported("instruction subject not lowered"),
        }
    }
    // A shared grammatical subject announces its targets once, even with several verbs.
    fn clause_subject(&mut self, clause: &Clause, support: &Support<'_>, binder: &mut Binder) -> Res<action::Operand> {
        self.clause_subject_scoped(clause, support, binder, false)
    }
    fn clause_subject_scoped(&mut self, clause: &Clause, support: &Support<'_>, binder: &mut Binder, dealer: bool) -> Res<action::Operand> {
        // the player the clause was said of, as named when it began: whose "their" is theirs
        if let Some(actor) = binder.actor {
            binder.clause = Some(expr::Subject::Local(actor));
            return Ok(action::Operand::Reference(actor));
        }
        if let Some((subject, ordinary)) = &self.clause_subject {
            if !dealer { ordinary.clone()?; }
            return Ok(subject.clone());
        }
        // A dealer-exempt reference is only reusable by an ordinary verb if ordinary
        // lookup already succeeded in this original context. Recipients named by the
        // first verb must not become evidence for reuse of a different cached operand.
        // Validate direct anaphors read-only, before any target/capture announcement.
        fn ordinary(subject: Option<&Subject>, support: &Support<'_>, binder: &Binder) -> Res<()> {
            match subject {
                Some(Subject::Selector(s) | Subject::Each(s)) if matches!(s.quant, Quant::Definite(Ref::Anaphor(_))) => {
                    binder.referent(s, support, true).map(|_| ())
                }
                Some(Subject::And { items, .. }) => items.iter().try_for_each(|s| ordinary(Some(s), support, binder)),
                _ => Ok(()),
            }
        }
        let ordinary = if dealer { ordinary(clause.subject.as_ref(), support, binder) } else { Ok(()) };
        let mut subject = self.subject_scoped(clause.subject.as_ref(), support, binder, dealer)?;
        if let Some(next) = self.contrast.clone() && matches!(subject, action::Operand::All(_)) {
            let out = self.slot();
            self.instrs.push(Instr::Capture { operand: subject, out });
            binder.contrast = Some((out, next));
            subject = action::Operand::Reference(out);
        }
        // An explicitly named owner/controller is a player antecedent, not just
        // a projection recomputed after this clause moves the object (608.2c).
        if matches!(clause.subject.as_ref(), Some(Subject::Selector(s))
            if s.set.domain == Domain::Player && matches!(s.quant, Quant::Definite(Ref::OwnerOf(_) | Ref::ControllerOf(_)))) {
            subject = self.keep(subject, ObjectSet::of(Domain::Player), 1, binder);
        }
        self.clause_subject = Some((subject.clone(), ordinary));
        binder.clause = match &subject {
            action::Operand::Reference(slot) => Some(expr::Subject::Local(*slot)),
            action::Operand::Source => Some(expr::Subject::Source),
            action::Operand::Subject(expr::Subject::Attached)
                if matches!(clause.subject.as_ref(), Some(Subject::Selector(s)) if kinds(&s.set) == (false, true)) => Some(expr::Subject::Attached),
            _ => None,
        };
        Ok(subject)
    }

}
fn quantity(n: &Amount, support: &Support<'_>, binder: &Binder) -> Res<expr::Amount> {
    expr::Amount::compile(n, support, &mut binder.scope(support))
}

fn player_quantity(n: &Amount, support: &Support<'_>, binder: &Binder) -> Res<expr::Amount> {
    expr::Amount::compile(n, support, &mut Scoped { binder, support, actor: true, object_actor: false, to_players: false })
}

/// The nearest duration belongs to every coordinated verb in its scope.
fn duration_mods(clause: &Clause, act: &ir::effect::Act, enclosing: &[&ir::effect::Duration]) -> Vec<ir::effect::Modifier> {
    let at = if std::ptr::eq(act, &clause.first) { 0 } else {
        clause.rest.iter().position(|(_, a)| std::ptr::eq(a, act)).expect("act belongs to clause") + 1
    };
    let stated = act.mods.iter().chain(clause.rest.iter().skip(at).flat_map(|(_, a)| &a.mods))
        .find_map(|m| if let ir::effect::Modifier::Duration(d) = m { Some(d) } else { None });
    stated.or_else(|| enclosing.last().copied()).map(|d| ir::effect::Modifier::Duration(d.clone())).into_iter().collect()
}

/// Lower every act in sequence. Unsupported acts are errors, never an ownership fallback.
pub(super) fn lower(effect: &Effect, support: &Support<'_>) -> Res<Program> {
    lower_scoped(effect, support, Binder::default(), Vec::new())
}
/// Whether the clause is of that sentence of the text.
fn says(sentence: &Sentence, clause: &Clause) -> bool {
    match sentence {
        Sentence::Do(body) => std::iter::once(&body.first).chain(body.rest.iter().map(|(_, c)| c)).any(|c| std::ptr::eq(c, clause)),
        Sentence::Led { rest, .. } => says(rest, clause),
        _ => false,
    }
}

/// A trailing "where X" defines the coordinated instruction body, including
/// clauses with different subjects ("each opponent loses X and you gain X").
/// Separate definitions keep their own clause scope; separate sentences never
/// borrow a definition from one another (107.3c, 608.2c/h).
fn clause_x<'a>(sentence: &'a Sentence, clause: &'a Clause) -> Option<(&'a Amount, &'a ir::effect::Act)> {
    let body = match sentence {
        Sentence::Do(body) => body,
        Sentence::Led { rest, .. } => return clause_x(rest, clause),
        _ => return None,
    };
    let clauses: Vec<_> = std::iter::once(&body.first).chain(body.rest.iter().map(|(_, c)| c)).collect();
    if !clauses.iter().any(|c| std::ptr::eq(*c, clause)) { return None; }
    let definitions: Vec<_> = clauses.iter().flat_map(|c| std::iter::once(&c.first).chain(c.rest.iter().map(|(_, a)| a)))
        .flat_map(|a| &a.mods).filter_map(|m| if let ir::Modifier::WhereX(n) = m { Some(n) } else { None }).collect();
    if let [definition] = definitions.as_slice() {
        let last = clauses.last().expect("a body has a first clause");
        return Some((definition, last.rest.last().map_or(&last.first, |(_, a)| a)));
    }
    // Multiple explicitly stated values do not become one shared definition.
    std::iter::once(&clause.first).chain(clause.rest.iter().map(|(_, a)| a))
        .flat_map(|a| &a.mods).find_map(|m| if let ir::Modifier::WhereX(n) = m {
            Some((n, clause.rest.last().map_or(&clause.first, |(_, a)| a)))
        } else { None })
}

/// A condition before any instructions have run can reference the trigger's schema,
/// and linked choices, but no local result that does not yet exist.
pub(crate) fn initial_condition(c: &ir::Condition, support: &Support<'_>) -> Res<condition::Condition> {
    condition::Condition::compile(c, support, &mut Binder::default().scope(support))
}

/// A single quantity operation under "for each" uses the same scaling as its suffix
/// form (608.2f). This introduces no per-member variable, loop or repeated operation.
fn quantity_leads(effect: &mut Effect) -> Res<()> {
    fn normalize(sentence: &mut Sentence) -> Res<()> {
        if let Sentence::Led { lead, rest } = sentence {
            if let Lead::Modifier(ir::Modifier::ForEach(amount)) = lead {
                if !matches!(amount, Amount::Count(set) if set.domain == Domain::Player)
                    && value_iteration::life_or_top(rest).is_none() {
                    let Sentence::Do(body) = &mut **rest else { return unsupported("for-each quantity lead: a single ordinary action required"); };
                    let clause = &mut body.first;
                    if !body.rest.is_empty() || !clause.rest.is_empty() || clause.may || clause.also || clause.first.may
                        || !matches!(clause.first.action, Action::Draw(_) | Action::DrawAdditional(_) | Action::Mill(_) | Action::GainLife(_) | Action::LoseLife(_) | Action::DealDamage(_) | Action::PutCounters(_) | Action::GetCounters { .. } | Action::Create(_)) {
                        return unsupported("for-each quantity lead: compound, optional or non-quantity action");
                    }
                    clause.first.mods.push(ir::Modifier::ForEach(amount.clone()));
                    *sentence = (**rest).clone();
                    return Ok(());
                }
            }
            normalize(rest)?;
        }
        Ok(())
    }
    for sentence in &mut effect.0 { normalize(sentence)?; }
    Ok(())
}

/// An explicit move origin qualifies every targeted card of that instruction
/// ("Return from your graveyard ... target creature cards"). It belongs on
/// those selectors for both announcement and resolution legality (601.2c, 608.2b).
fn targeted_move_origins(effect: &mut Effect) -> Res<()> {
    fn sentence(said: &mut Sentence) -> Res<()> {
        match said {
            Sentence::Led { rest, .. } => sentence(rest)?,
            Sentence::Do(body) => for act in acts_mut(body) {
                let Action::Move { moves, from: Some(from), .. } = &mut act.action else { continue };
                if from.zones.items != [ir::Zone::Graveyard] || from.slice.is_some()
                    || !from.owner.as_deref().is_some_and(crate::select::is_you)
                    || moves.iter().any(|(objects, _)| objects.items.is_empty() || objects.items.iter().any(|s|
                        !matches!(s.quant, Quant::Target(_)) || s.set.domain != Domain::Card
                            || s.set.zone.as_ref().is_some_and(|zone| zone != from))) {
                    return unsupported("move origin: targeted cards in your graveyard required");
                }
                for (objects, _) in moves { for selector in &mut objects.items { selector.set.zone = Some(from.clone()); } }
                if let Action::Move { from, .. } = &mut act.action { *from = None; }
            },
            _ => {},
        }
        Ok(())
    }
    for said in &mut effect.0 { sentence(said)?; }
    Ok(())
}

/// Token anaphors describe the creating instruction's template (111.3), not the
/// characteristics or existence of a token previously created. Bind only one explicit
/// earlier template; conditional counts therefore reuse it without a runtime search.
fn token_references(effect: &mut Effect) -> Res<()> {
    /// What "those tokens" are: the earlier template, and how it said they enter ("create two
    /// tapped 2/2 black Horror creature tokens. … instead create X of those tokens": tapped).
    type Template = (ir::token::TokenKind, bool, Option<ir::token::Combat>);
    fn visit(said: &mut Sentence, earlier: &mut Vec<Template>) -> Res<()> {
        match said {
            Sentence::Led { rest, .. } => visit(rest, earlier)?,
            Sentence::Do(body) => for act in acts_mut(body) {
                if let Action::Create(tokens) = &mut act.action {
                    for token in &mut tokens.items {
                        if token.kind == ir::token::TokenKind::Those {
                            let [(kind, tapped, combat)] = earlier.as_slice() else {
                                return unsupported("token reference: absent or ambiguous earlier token template");
                            };
                            token.kind = kind.clone();
                            token.tapped |= *tapped;
                            if token.combat.is_none() { token.combat = combat.clone(); }
                        } else {
                            let template = (token.kind.clone(), token.tapped, token.combat.clone());
                            if !earlier.contains(&template) { earlier.push(template); }
                        }
                    }
                }
            },
            _ => {},
        }
        Ok(())
    }
    let mut earlier = Vec::new();
    for said in &mut effect.0 { visit(said, &mut earlier)?; }
    Ok(())
}

/// "If this land isn't a creature, it becomes a 2/4 Wizard creature": opening the text, the
/// condition names the ability's own object and nothing else was said before, so "it" is
/// that object. The sentence with "it" said as the object it is.
fn conditioned_self(sentence: &Sentence) -> Option<Sentence> {
    let Sentence::Led { lead: Lead::Modifier(ir::Modifier::If(ir::Condition::Is { what, .. })), rest } = sentence else { return None };
    let Sentence::Do(body) = rest.as_ref() else { return None };
    if what.quant != Quant::Definite(Ref::This) { return None; }
    let Some(Subject::Selector(it)) = &body.first.subject else { return None };
    if it.quant != Quant::Definite(Ref::Anaphor(ir::Anaphor::It)) { return None; }
    let mut said = sentence.clone();
    if let Sentence::Led { rest, .. } = &mut said && let Sentence::Do(body) = rest.as_mut() {
        body.first.subject = Some(Subject::Selector((**what).clone()));
    }
    Some(said)
}

/// The acts of a sentence, in the order they are said.
fn acts_mut(body: &mut ir::effect::Body) -> impl Iterator<Item = &mut ir::effect::Act> {
    std::iter::once(&mut body.first).chain(body.rest.iter_mut().map(|(_, c)| c))
        .flat_map(|c| std::iter::once(&mut c.first).chain(c.rest.iter_mut().map(|(_, a)| a)))
}
fn says_again(sentence: &Sentence) -> bool {
    match sentence {
        Sentence::Led { rest, .. } => says_again(rest),
        Sentence::Do(body) => std::iter::once(&body.first).chain(body.rest.iter().map(|(_, c)| c))
            .flat_map(|c| std::iter::once(&c.first).chain(c.rest.iter().map(|(_, a)| a)))
            .any(|a| a.mods.contains(&ir::Modifier::Again)),
        _ => false,
    }
}
/// "It explores, then it explores again": "again" restates an act the sentence did before,
/// which is done once more as said. Dropped there; anywhere else it is kept, and refused.
fn restated(effect: &mut Effect) {
    fn sentence(s: &mut Sentence) {
        match s {
            Sentence::Led { rest, .. } => sentence(rest),
            Sentence::Do(body) => {
                let mut said: Vec<Action> = Vec::new();
                for act in acts_mut(body) {
                    if act.mods.contains(&ir::Modifier::Again) && said.contains(&act.action) {
                        act.mods.retain(|m| *m != ir::Modifier::Again);
                    }
                    said.push(act.action.clone());
                }
            }
            _ => {}
        }
    }
    for s in &mut effect.0 { sentence(s); }
}

/// "Kellan gains \"Whenever Kellan deals combat damage …\"": an ability an object gives
/// itself names it, as the granting object, by its name (201.5a). What does the triggered
/// abilities' events, said so, is "this" object; none where nothing is said so.
fn granted_to_itself(action: &Action) -> Option<Action> {
    fn trigger(t: &mut ir::Trigger) -> bool {
        match t {
            ir::Trigger::Event { pattern, .. } => pattern.subjects.iter_mut().fold(false, |any, s| {
                if s.quant != Quant::Definite(Ref::Granting) { return any; }
                s.quant = Quant::Definite(Ref::This);
                true
            }),
            ir::Trigger::Any(all) => all.iter_mut().fold(false, |any, t| trigger(t) || any),
            _ => false,
        }
    }
    let mut action = action.clone();
    let Action::Gains(granted) = &mut action else { return None };
    let mut changed = false;
    for g in granted {
        if let ir::action::Granted::Ability(a) = g && let ir::Ability::Triggered { trigger: t, .. } = a.as_mut() { changed |= trigger(t); }
    }
    changed.then_some(action)
}

/// What lowering a definition's text has done, shared with the lowering of the texts within
/// it: how many lowerings of text it entered, and of refused ones the instructions they never
/// reached, which may hide further refusals.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Lowering { entered: usize, unreached: usize }

/// How far one lowering of text got: the instructions of each of its steps, once read, as
/// (its own, those of the texts within it, which are lowered on their own); the step being
/// lowered, none before the first and all of them once lowered; how many lowerings had been
/// entered as it began; and the step lowering resumes at, past those a step took with it.
#[derive(Default)]
struct Reach { steps: Vec<(usize, usize)>, at: Option<usize>, entered: usize, skip_through: usize }

impl Reach {
    /// The instructions of `effect` its refused lowering never reached: all of them when it
    /// refused before lowering its first step; else those of the steps after the one refused,
    /// of the texts within that one if their lowering was never entered (when entered, theirs
    /// counts what it left), and the activation restrictions read once the steps are lowered.
    fn unreached(&self, effect: &Effect, entered: usize) -> usize {
        let Some(at) = self.at else { return instructions(&effect.0) };
        let after: usize = self.steps.iter().skip(self.skip_through.max(at + 1)).map(|(own, within)| own + within).sum();
        let within = self.steps.get(at).filter(|_| entered == self.entered).map_or(0, |(_, within)| *within);
        let restrictions = if at < self.steps.len() { effect.0.iter().filter(|s| matches!(s, Sentence::Restrictions(_))).count() } else { 0 };
        after + within + restrictions
    }
}

/// How many instructions sentences say: each act, and each rule or restriction they state.
pub(crate) fn instructions(sentences: &[Sentence]) -> usize {
    sentences.iter().map(|sentence| match sentence {
        Sentence::Do(body) => std::iter::once(&body.first).chain(body.rest.iter().map(|(_, c)| c)).map(|c| 1 + c.rest.len()).sum(),
        Sentence::Led { rest, .. } => instructions(std::slice::from_ref(&**rest)),
        Sentence::Static(_) | Sentence::Restrictions(_) | Sentence::SameIsTrueFor(_) => 1,
        _ => 0,
    }).sum()
}

impl TextStep<'_> {
    /// The instructions this step lowers: (its own, those of the texts within it).
    fn instructions(&self) -> (usize, usize) {
        let all = |rest: &Sentence, more: &[Sentence]| instructions(std::slice::from_ref(rest)) + instructions(more);
        match self {
            TextStep::Act(..) | TextStep::Rule(_) | TextStep::CastRider(_) | TextStep::ExiledInstead(_) | TextStep::KeywordCost(..) => (1, 0),
            TextStep::Alternative(clause) => (1 + clause.rest.len(), 0),
            TextStep::CardTypes(rest) | TextStep::CardPayments(_, rest) => (all(rest, &[]), 0),
            TextStep::Reflexive(rest, more, _, _) | TextStep::Later(_, _, rest, more, _) => (0, all(rest, more)),
            TextStep::Delayed(_, _, _, rest, _) | TextStep::ManaSpent(_, rest, _) => (0, all(rest, &[])),
            _ => (0, 0),
        }
    }
}

/// Lowering stops at its first refusal: of one refused, what it never reached is added to
/// what its definition's lowering left unreached ([`Lowering`]).
fn lower_scoped(effect: &Effect, support: &Support<'_>, binder: Binder, initial: Vec<Option<Kind>>) -> Res<Program> {
    let lowering = support.lowering();
    lowering.set(Lowering { entered: lowering.get().entered + 1, ..lowering.get() });
    let mut reach = Reach::default();
    let lowered = lower_steps(effect, support, binder, initial, &mut reach);
    if lowered.is_err() { unreached(support, reach.unreached(effect, lowering.get().entered)); }
    lowered
}

/// Instructions a refused lowering of the definition's text never reached.
fn unreached(support: &Support<'_>, instructions: usize) {
    let lowering = support.lowering();
    lowering.set(Lowering { unreached: lowering.get().unreached + instructions, ..lowering.get() });
}

fn lower_steps(effect: &Effect, support: &Support<'_>, mut binder: Binder, initial: Vec<Option<Kind>>, reach: &mut Reach) -> Res<Program> {
    // "amass Orcs 1" is what 701.47a says it means; "bolster 2", "support 2", "blight 1"
    // what 701.39a, 701.41a and 701.68a say
    let numbered = crate::numbered::expand(effect, support.spell_text());
    let mut expanded = match crate::amass::expand(&numbered) {
        Cow::Borrowed(_) => numbered,
        Cow::Owned(e) => Cow::Owned(e),
    };
    fn needs_quantity_lead(sentence: &Sentence) -> bool {
        match sentence {
            Sentence::Led { lead: Lead::Modifier(ir::Modifier::ForEach(amount)), rest } =>
                !matches!(amount, Amount::Count(set) if set.domain == Domain::Player) || needs_quantity_lead(rest),
            Sentence::Led { rest, .. } => needs_quantity_lead(rest),
            _ => false,
        }
    }
    if expanded.0.iter().flat_map(sentence_acts).any(|a| matches!(a.action, Action::Move { from: Some(_), .. })) {
        targeted_move_origins(expanded.to_mut())?;
    }
    if expanded.0.iter().any(|s| matches!(s, Sentence::RoundEach { .. })) { rounding::apply(expanded.to_mut())?; }
    if expanded.0.iter().any(needs_quantity_lead) { quantity_leads(expanded.to_mut())?; }
    if expanded.0.iter().any(says_again) { restated(expanded.to_mut()); }
    if expanded.0.iter().flat_map(sentence_acts).any(|a| matches!(&a.action,
        Action::Create(tokens) if tokens.items.iter().any(|t| t.kind == ir::token::TokenKind::Those))) {
        token_references(expanded.to_mut())?;
    }
    if expanded.0.len() == effect.0.len() { binder.paragraphs = support.paragraph_starts().into_iter().map(|start| binder.sentences + start).collect(); }
    if let Some(first) = expanded.0.first() && let Some(own) = conditioned_self(first) { expanded.to_mut().0[0] = own; }
    let effect: &Effect = &expanded;
    let non_generic = crate::mana::non_generic(effect);
    if crate::ability_limit::Limit::of(effect)?.is_some() && !matches!(support.ability(), Some(ir::Ability::Triggered { .. })) {
        return unsupported("turn limit on a non-triggered body");
    }
    let inherited_antecedents = (binder.subjects.len(), binder.source_named);
    binder.completed_subject = CompletedSubject::Other; // fresh body: no creator/sibling context
    let acts = text_steps(effect, support)?;
    let past_reads = acts.iter().any(|step| matches!(step, TextStep::If(ir::Condition::Was(_), _)));
    reach.steps = acts.iter().map(TextStep::instructions).collect();
    // the clause each sentence begins with, under its leads
    fn head(sentence: &Sentence) -> Option<&Clause> {
        match sentence { Sentence::Do(body) => Some(&body.first), Sentence::Led { rest, .. } => head(rest), _ => None }
    }
    let heads: Vec<&Clause> = effect.0.iter().filter_map(head).collect();
    let base = binder.sentences;
    let mut e = Emit { instrs: Vec::new(), acts: Vec::new(), next: initial.len(), targets: Vec::new(), relative_targets: None, sentence_targets: 0, target_relations: Vec::new(), announced_mask: crate::announcement::ANY, alternative_kicker_targets: false, target_x: None, clause_subject: None, contrast: None, captured_source: None, unless_open: None, unless_said: None };
    fn group_subject(clause: &Clause) -> Option<&ObjectSet> {
        match clause.subject.as_ref()? {
            Subject::Set(set) if set.domain == Domain::Permanent => Some(set),
            Subject::Selector(s) | Subject::Each(s)
                if matches!(s.quant, Quant::All | Quant::Each) && s.set.domain == Domain::Permanent => Some(&s.set),
            _ => None,
        }
    }
    let mut clauses: Vec<&Clause> = Vec::new();
    for step in &acts {
        if let TextStep::Act(clause, ..) = step
            && clauses.last().is_none_or(|previous| !std::ptr::eq(*previous, *clause)) {
            clauses.push(*clause);
        }
    }
    // The same unqualified domain/ownership with a stated “other” is a group
    // contrast. Capture membership before the first effect changes characteristics.
    let contrasts: Vec<_> = clauses.windows(2).filter_map(|pair| {
        let (a, b) = (group_subject(pair[0])?, group_subject(pair[1])?);
        (b.other && !a.other && a.domain == b.domain && a.controller == b.controller
            && a.owner == b.owner && a.caster == b.caster && a.zone == b.zone)
            .then_some((pair[0], pair[1], b))
    }).collect();
    let mut skip_rider = false;
    let mut branches = Vec::new();
    // conditions said after their act, read once it has been: (branch, condition, what was
    // named before the act, the first target the act may declare)
    let mut deferred: Vec<(usize, &ir::Condition, Binder, usize)> = Vec::new();
    let mut previous_if = None;
    // How many groups and damage results there were before the branch an "otherwise" or
    // "instead" follows: what that branch produced is not done on the alternative's path.
    let mut untaken = (0, 0);
    let mut target_branches = Vec::new();
    let mut previous_target_if = None;
    // the next act is the one a sentence says is done instead
    let mut replacing = false;
    // the instead sentence being lowered declares alternative targets (601.2c)
    let mut alternative_targets = false;
    // the first target group an "otherwise" branch declares
    let mut otherwise_targets = 0;
    // an instead branch with alternative targets: its first target group, and its depth
    let mut alternative_close: Option<(usize, usize)> = None;
    let mut durations = Vec::new();
    let mut after_turns = Vec::new();
    let mut created_turn_scopes = Vec::new();
    let mut optional_groups = 0;
    let mut each_completed = None;
    // A trailing "where X is …" defines its coordinated instruction body (107.3c).
    // Compute it at the first use, retain it through that body, and never borrow
    // it from a separate sentence. Independent explicit definitions keep clause scope.
    struct WhereX<'a> { stated: Option<&'a Amount>, start: usize, gated: bool, branch_local: bool, last: bool, before: Option<expr::Amount> }
    let mut defining: Option<WhereX> = None;
    let definitions: Vec<_> = acts.iter().filter_map(|step| match step { TextStep::Act(_, act, _) => Some(act), _ => None })
        .filter(|act| act.mods.iter().any(|m| matches!(m, ir::effect::Modifier::WhereX(_)))).collect();
    let body_x = matches!(support.ability(), Some(ir::Ability::Triggered { .. }))
        && matches!(definitions.as_slice(), [act] if matches!(act.action, Action::PayLife(_) | Action::PayMana(_)));
    let mut x_defined = false;
    let mut x_scope: Option<&Amount> = None;
    for (index, part) in acts.iter().copied().chain(std::iter::once(TextStep::EndDuration)).enumerate() {
        if !created_turn_scopes.is_empty() && !matches!(part,TextStep::Rule(_)|TextStep::EndDuringCreatedTurn) {
            return unsupported("during a created turn: only a retained activation-family rule is implemented");
        }
        // an act said "unless" whose path did not end it does not follow the payment
        if e.unless_open.is_some() || e.unless_said.is_some() {
            let TextStep::Act(_, act, _) = acts[index - 1] else { unreachable!("only an act opens a payment") };
            return unsupported(format!("modifier: Unless of {}", crate::select::name(&act.action)));
        }
        // Complete once, before any lexical marker restores a scope. Looking behind
        // covers action paths that end early with continue and joined verbs as one clause.
        if index > 0 && index - 1 >= reach.skip_through {
            match acts[index - 1] {
                TextStep::Act(clause, act, _) if std::ptr::eq(act, clause.rest.last().map_or(&clause.first, |(_, a)| a)) => binder.complete_clause(e.captured_source),
                TextStep::Alternative(_) => binder.complete_clause(e.captured_source),
                _ => {},
            }
        }
        if let Some(w) = defining.take() {
            let read = binder.x.take().is_some_and(|read| read.get());
            if let Some(n) = w.stated && (read || w.last) {
                if w.branch_local && !w.last {
                    return unsupported("where X: first definition may be skipped before a later coordinated use");
                }
                let value = match w.before { Some(value) => value, None => quantity(n, support, &binder)? };
                e.define_x(value, w.start, w.gated, &mut binder)?;
                x_defined = true;
            }
            if x_defined && (!w.last || body_x) { e.acts.last_mut().expect("the act has ended").keeps_x = true; }
            if w.last && !body_x { x_defined = false; }
        }
        // Conditional/optional text markers emit their own ActSpan too. An already
        // initialized value survives those markers just as it survives an ordinary
        // coordinated action; the final consuming act cleared x_defined above.
        if x_defined && x_scope.is_some() {
            e.acts.last_mut().expect("a defined X has an earlier act").keeps_x = true;
        }
        if index == acts.len() { break; }
        if index < reach.skip_through { continue; }
        // what fails before here completes the step before; from here, this step is lowered
        reach.at = Some(index);
        reach.entered = support.lowering().get().entered;
        if !after_turns.is_empty() && matches!(part,TextStep::Rule(_) | TextStep::Delayed(..) | TextStep::Alternative(_)) {
            return unsupported("after turn: only extra turns are scheduled at that boundary");
        }
        // what is said between acts (a condition, a lead) is of no sentence yet
        match part {
            TextStep::Act(clause, ..) | TextStep::Alternative(clause) => binder.at = (effect.0.iter().position(|s| says(s, clause)).map_or(usize::MAX, |i| base + i), index),
            _ => { binder.clause = None; binder.at = (usize::MAX, index); }
        }
        let (clause, act, alone) = match part {
            TextStep::ManaGenericForbidden => {
                if !non_generic { return unsupported("mana: non-generic spending rider needs one unconditional producer"); }
                continue;
            }
            TextStep::CardPayments(set, rest) => {
                if !durations.is_empty() || optional_groups != 0 || binder.each.is_some() {
                    return unsupported("per-card payment: nested, lasting or optional scope");
                }
                value_iteration::card_payments(set, rest, support, &mut e, &mut binder)?;
                continue;
            }
            TextStep::CardTypes(rest) => {
                value_iteration::hand_selection(rest, support, &mut e, &mut binder)?;
                e.end_act(true, None);
                continue;
            }
            TextStep::May(said_of) => {
                if matches!(said_of.rest.last().map(|(_,a)| &a.action), Some(Action::Numbered(ir::NumberedAction::CollectEvidence, _))) {
                    if !controller_only(said_of.subject.as_ref()) || optional_groups != 0 || binder.each.is_some() {
                        return unsupported("collect evidence compound: one controller's optional payment required");
                    }
                    let players = e.clause_subject(said_of, support, &mut binder)?;
                    let mut actions = Vec::new();
                    for act in std::iter::once(&said_of.first).chain(said_of.rest.iter().map(|(_,a)| a)) {
                        if act.may || !act.mods.is_empty() { return unsupported("collect evidence compound: individually optional or modified actions"); }
                        match &act.action {
                            Action::Exile(objects) if objects.items.iter().all(|s| matches!(s.quant, Quant::Definite(_) | Quant::Target(_))) => {
                                let named = e.objects(objects, support, &mut binder)?;
                                actions.push(choice::Action::exile(players.clone(), named));
                            },
                            Action::Numbered(ir::NumberedAction::CollectEvidence, n) => {
                                let (_, cards) = crate::cost::evidence_cost();
                                let set = expr::Set::compile(&cards.set, support, &mut binder.scope(support))?;
                                actions.push(choice::Action::collect_evidence(players.clone(), set, quantity(n, support, &binder)?));
                            },
                            _ => return unsupported("collect evidence compound: deterministic named exile prefix required"),
                        }
                    }
                    let action = choice::Action::together(actions)?;
                    action.payment_ok()?;
                    let (paid, possible) = (e.slot(), e.slot());
                    binder.outcome = Some((players, paid, possible, e.instrs.len()));
                    e.instrs.push(Instr::Choice { action, optional: true, payment: true, paid, possible, chosen: None, outcomes: None, actual: None, discarded_before: None });
                    e.end_act(true, None);
                    reach.skip_through = index + said_of.rest.len() + 3;
                    continue;
                }
                optional_groups += 1;
                // the player the clause is said of chooses, and does what it says: named once,
                // as the clause begins (608.2h: "its controller" of what was exiled is the
                // player who controlled it as it last existed)
                let who = if controller_only(said_of.subject.as_ref()) { Who::Controller } else {
                    let player = e.subject(said_of.subject.as_ref(), support, &mut binder)?;
                    let actor = e.slot();
                    e.instrs.push(Instr::Capture { operand: player, out: actor });
                    Who::Of(actor)
                };
                let consent = e.slot();
                let probe = if let Action::Reveal(objects) = &said_of.first.action
                    && let [selector] = objects.items.as_slice()
                    && let Some(mention) = mention(selector)
                    && let Taking::Exactly(n) = mention.taking
                {
                    let set = binder.selection(&mention.set, support)?;
                    e.unite(&mention.reference, &mut binder);
                    let group = binder.resolve(&mention.reference)?;
                    Some(Probe { from: group.slot, set, count: expr::Amount::constant(n as u32), still_in: group.zone })
                } else { None };
                e.instrs.push(Instr::Gate { who, probe, out: consent, possible: None });
                branches.push((e.instrs.len(), binder.clone(), consent));
                if let Who::Of(actor) = who { binder.actor = Some(actor); }
                e.instrs.push(Instr::Branch { condition: condition::Condition::stored(consent, true), end: 0, remember: None, skipped_values: Vec::new() });
                e.end_act(false, None);
                continue;
            }
            TextStep::EndMay => {
                optional_groups -= 1;
                let (at, mut outer, _) = branches.pop().expect("optional scope");
                close_branch(&mut e.instrs, at, &binder, &mut outer);
                outer.subjects = binder.subjects.clone();
                binder = outer;
                continue;
            }
            TextStep::ManaSpent(on, rest, whole) => {
                if !durations.is_empty() { return unsupported("spending trigger: stated duration not retained"); }
                let Some(Instr::Mana { production, .. }) = e.instrs.last() else {
                    return unsupported("spending trigger: no immediately preceding mana production");
                };
                if production.has_spending_trigger() || e.instrs.iter().filter(|i|matches!(i,Instr::Mana{..})).count()!=1 { return unsupported("spending trigger: ambiguous produced mana reference"); }
                let prefix = Program::with_initial(e.instrs.clone(), e.acts.clone(), e.targets.clone(), initial.clone())?;
                let mut scope=binder.clone(); scope.sentences=base+effect.0.len();scope.clause=None;scope.outcome=None;scope.turn=None;scope.condition_amount=None;
                // the delayed ability's text is its own sentence, "When that mana is spent …"
                // (603.3, 603.7a), said beside the mana ability that creates it (106.6)
                let program=said(lower_scoped(&Effect(vec![rest.clone()]), support, scope, prefix.kinds)?, whole, &[]);
                let Some(Instr::Mana { production, .. })=e.instrs.last_mut() else {unreachable!()};
                production.when_spent(on,program,support)?;
                continue;
            }
            TextStep::Reflexive(rest, following, happened, whole) => {
                if !durations.is_empty() || optional_groups != 0 { return unsupported("reflexive: duration or optional compound scope"); }
                // "When it connives this way", "When you search your library this way": an
                // instruction this resolution follows whatever it finds or discards (701.50,
                // ruling 2022-04-29; 701.23)
                let always = |pattern: &ir::EventPattern| {
                    let [who] = pattern.subjects.as_slice() else { return None };
                    if *pattern != ir::event::EventPattern::new(pattern.subjects.clone(), pattern.actions.clone()) { return None; }
                    match pattern.actions.as_slice() {
                        [ir::EventAction::Deed(d)] if d.verb == ir::Verb::Connive && !d.passive && d.args.is_empty()
                            && who.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::It)) => Some(ir::Verb::Connive),
                        [ir::EventAction::Searches { zone }] if who.quant == Quant::Definite(Ref::You)
                            && zone.owner_of(ir::Zone::Library).is_some_and(|o| o.quant == Quant::Definite(Ref::You)) => Some(ir::Verb::Search),
                        _ => None,
                    }
                };
                let mut each = None;
                let when = if let Some(verb) = happened.and_then(always) {
                    // instructed once, and not only on a path the resolution may not take
                    let of = |i: &Instr| match verb { ir::Verb::Connive => matches!(i, Instr::Connive { .. }), _ => matches!(i, Instr::Search { by: Who::Controller, .. }) };
                    if e.instrs.iter().filter(|i| of(i)).count() != 1 || e.instrs.iter().any(|i| matches!(i, Instr::Branch { .. } | Instr::Gate { .. })) {
                        return unsupported("reflexive: not one instruction it is said of, done whatever happens");
                    }
                    condition::Condition::always()
                } else if let Some((verb, what, by_you, into)) = happened.and_then(condition::this_way_members)
                    && what.quant == Quant::Indefinite(Count::Exactly(Amount::Const(1))) && what.chooser.is_none() && what.random.is_none() {
                    // "When you discard a nonland card this way": once for each card discarded
                    // that is one (603.12a, 603.2c)
                    let members = expr::Number::members(&what.set, Some(verb), into, by_you, support, &mut binder.scope(support))?;
                    each = Some((ReflexiveEach { members, member: e.slot() }, what.set.clone()));
                    condition::Condition::always()
                } else if let Some(pattern) = happened {
                    // what happened this way, as "if you discarded a nonland card this way" asks
                    let asked = ir::Condition::Happened { pattern: pattern.clone(), window: Some(ir::condition::Window::ThisWay), times: None, tense: ir::event::Tense::Past, negated: false };
                    condition::Condition::compile(&asked, support, &mut binder.scope(support))?
                } else {
                    let Some((actor, paid, _, instruction)) = binder.outcome.clone() else { return unsupported("reflexive: no earlier payment producer"); };
                    if actor != action::Operand::Controller { return unsupported("reflexive: producer must be this resolution’s controller"); }
                    // 603.12: it triggers when the producer is done, as "if you do" asks (118.12)
                    e.producer(instruction)?;
                    condition::Condition::stored(paid, true)
                };
                e.end_act(false, None);
                let prefix = Program::with_initial(e.instrs.clone(), e.acts.clone(), e.targets.clone(), initial.clone())?;
                let mut kinds = prefix.kinds;
                let mut scope = binder.clone();
                scope.sentences = base + effect.0.len(); scope.clause = None; scope.outcome = None;
                scope.turn = None; scope.created_turn = None; scope.condition_amount = None; scope.searched = None; scope.found_waiting = false;
                // "that card" is the one it triggered for
                if let Some((each, set)) = &each {
                    if kinds.len() <= each.member { kinds.resize(each.member + 1, None); }
                    kinds[each.member] = Some(Kind::Subjects);
                    scope.result = Some((each.member, set.clone(), 1));
                }
                let mut body = vec![rest.clone()];
                for sentence in following {
                    if let Some(change) = crate::announcement::activation_change(sentence, support) { change?; continue; }
                    match sentence {
                        Sentence::Restrictions(_) => {
                            if !matches!(support.ability(), Some(ir::Ability::Activated { .. })) {
                                return unsupported("activation restriction on non-activated effect");
                            }
                        },
                        Sentence::Rider(ir::effect::Rider::TriggersOnlyOnceEachTurn | ir::effect::Rider::DoThisOnlyOnceEachTurn) => {},
                        _ => body.push(sentence.clone()),
                    }
                }
                let program = said(lower_scoped(&Effect(body.clone()), support, scope, kinds)?, whole, &body[1..]);
                e.instrs.push(Instr::Reflexive { when, each: each.map(|(each, _)| each), program });
                e.end_act(false, None);
                continue;
            }
            TextStep::Delayed(pattern, limit, next, rest, whole) => {
                // how long it watches (603.7b): once, unless the text says until when
                let stated: &[&ir::effect::Duration] = &durations;
                let lasts = match (stated, limit) {
                    ([], None) => crate::state::Lasts::Indefinite,
                    ([], Some(ir::DelayLimit::ThisTurn)) | ([ir::effect::Duration::ThisTurn], None) => crate::state::Lasts::ThisTurn,
                    ([ir::effect::Duration::Until(ir::event::Time { end: true, step: ir::event::Step::Turn, whose: ir::event::StepOwner::This })], None) => crate::state::Lasts::ThisTurn,
                    ([ir::effect::Duration::Until(ir::event::Time { end: false, step: ir::event::Step::Turn, whose: ir::event::StepOwner::Player { who, next: true } })], None)
                        if crate::select::is_you(who) => crate::state::Lasts::UntilYourNextTurn,
                    _ => return unsupported("delayed lead: a duration the engine does not keep it for"),
                };
                // "whenever that creature attacks": of the particular objects named as it is
                // made (603.7c). "whenever a creature an opponent controls attacks": of
                // whatever is so as the event happens, which its own words then name.
                let bound_subjects = !pattern.subjects.is_empty() && pattern.subjects.iter().all(|s| matches!(s.quant, Quant::Definite(Ref::Anaphor(_))));
                let described = !pattern.subjects.is_empty() && pattern.subjects.iter().all(|s| matches!(s.quant, Quant::Indefinite(_)));
                let once = next || lasts == crate::state::Lasts::Indefinite;
                let next_event = next && !pattern.subjects.is_empty();
                if !bound_subjects && !(described && lasts != crate::state::Lasts::Indefinite) && !next_event {
                    return unsupported("delayed lead requires an explicitly bound object subject");
                }
                let event = pattern;
                let (subjects, pattern) = if bound_subjects {
                    (pattern.subjects.iter().map(|s| e.operand(s, support, &mut binder)).collect::<Res<Vec<_>>>()?,
                        crate::trigger::DelayedPattern::event(crate::trigger::Occurrence::compile(pattern, support)?))
                } else {
                    (Vec::new(), crate::trigger::DelayedPattern::any(crate::trigger::Pattern::compile(pattern, support)?))
                };
                let nested = support.clone().with_event(event);
                let support = if bound_subjects { support } else { &nested };
                e.end_act(false, None);
                let prefix = Program::with_initial(e.instrs.clone(), e.acts.clone(), e.targets.clone(), initial.clone())?;
                let mut captured_scope = binder.clone();
                captured_scope.sentences = base + effect.0.len();
                captured_scope.clause = None;
                captured_scope.outcome = None;
                captured_scope.turn = None;
                captured_scope.condition_amount = None;
                captured_scope.created_turn = None; // future-turn anchors cannot escape into a later resolution
                captured_scope.searched = None;
                captured_scope.found_waiting = false;
                // The following optional new-target sentence modifies the delayed copy,
                // not the registration's already-resolving ability (707.10c).
                let mut body = vec![rest.clone()];
                if let Some(TextStep::Act(clause, act, true)) = acts.get(index + 1)
                    && matches!(&act.action, Action::ChooseNewTargets { for_, one: false }
                        if matches!(for_.items.as_slice(), [s] if matches!(s.quant,
                            Quant::Definite(Ref::Anaphor(ir::Anaphor::Copy { .. } | ir::Anaphor::ThatCopy)))))
                    && (clause.may || act.may) && controller_only(clause.subject.as_ref())
                    && act.mods.is_empty()
                    && let Some(sentence) = effect.0.iter().find(|sentence| matches!(sentence,
                        Sentence::Do(body) if body.rest.is_empty() && clause.rest.is_empty()
                            && std::ptr::eq(&body.first, *clause)))
                {
                    body.push(sentence.clone());
                    reach.skip_through = index + 2;
                }
                let program = said(lower_scoped(&Effect(body.clone()), support, captured_scope, prefix.kinds)?, whole, &body[1..]);
                e.instrs.push(Instr::Delayed { pattern, subjects, program, lasts, once });
                e.end_act(false, None);
                continue;
            }
            // what an opening-hand action has its player do at a later step: a delayed
            // triggered ability made now, running all those sentences then (603.7, 103.6)
            TextStep::Later(step, whose, rest, more, whole) => {
                if !durations.is_empty() { return unsupported("delayed step: with a duration"); }
                let pattern = crate::trigger::DelayedPattern::step(step, whose, support)?;
                e.end_act(false, None);
                let prefix = Program::with_initial(e.instrs.clone(), e.acts.clone(), e.targets.clone(), initial.clone())?;
                let mut scope = binder.clone();
                scope.sentences = base + effect.0.len(); scope.clause = None; scope.outcome = None;
                // Like other delayed bodies, this is a new future resolution.
                // Unrepresented future-turn anchors cannot escape into it.
                scope.turn = None; scope.created_turn = None; scope.condition_amount = None;
                scope.searched = None; scope.found_waiting = false;
                let body = Effect(std::iter::once(rest.clone()).chain(more.iter().cloned()).collect());
                let program = said(lower_scoped(&body, support, scope, prefix.kinds)?, whole, more);
                e.instrs.push(Instr::Delayed { pattern, subjects: Vec::new(), program, lasts: crate::state::Lasts::Indefinite, once: true });
                e.end_act(false, None);
                continue;
            }
            TextStep::If(c, target_scope) => {
                // A singular created-token noun denotes the complete replacement result.
                // This bounded type-condition/continuous-change composition tests each
                // member, freezing the matching subset without changing later antecedents.
                if let ir::Condition::Is {what,pred:ir::condition::Predicate::A(set),negated}=c
                    && (matches!(&set.filter,ir::Filter::Atom(ir::Atom::Type(_)))
                        || matches!(&set.filter,ir::Filter::All(parts) if matches!(parts.as_slice(),[ir::Filter::Atom(ir::Atom::Type(_))])))
                    && set.domain==Domain::Permanent && set.zone.is_none() && set.controller.is_none()
                    && set.owner.is_none() && set.caster.is_none() && !set.other
                    && let Ok(expr::Subject::Local(source))=expr::Subject::compile(what,&mut binder.scope(support))
                    && binder.created_groups.contains(&source)
                    && let Some([TextStep::Act(clause,act,true),TextStep::EndIf])=acts.get(index+1..index+3)
                    && !matches!(acts.get(index+3),Some(TextStep::Otherwise | TextStep::Instead(_)))
                    && matches!(&act.action,Action::Becomes(ir::action::Becoming::Is(_)))
                    && !clause.may && !act.may && clause.rest.is_empty()
                    && let Some(Subject::Selector(subject))=&clause.subject
                    && expr::Subject::compile(subject,&mut binder.scope(support))?==expr::Subject::Local(source)
                {
                    let filter=expr::Filter::compile(&set.filter,support,&mut binder.scope(support))?;
                    let filter=if *negated {expr::Filter::Not(Box::new(filter))} else {filter};
                    let out=e.slot();
                    e.instrs.push(Instr::Capture {operand:action::Operand::Matching(action::Matching {of:Box::new(action::Operand::Reference(source)),filter:Box::new(filter),controller:None}),out});
                    let outer=binder.clone();
                    binder.result=Some((out,ObjectSet::of(Domain::Permanent),u32::MAX));
                    binder.created_single=Some(out);binder.created_groups.push(out);
                    target_branches.push((e.announced_mask,(crate::announcement::ANY,crate::announcement::ANY)));
                    let remember=e.slot();
                    branches.push((e.instrs.len(),outer,remember));
                    e.instrs.push(Instr::Branch {condition:condition::Condition::always(),end:0,remember:Some(remember),skipped_values:Vec::new()});
                    e.end_act(false,None);
                    continue;
                }
                // "Counter target spell if it's blue": the condition is said after its act, of
                // what that act names. It is read once the act has been (`EndIf`), seeing what
                // was named before the act and the act's own targets, and nothing named later;
                // its branch still comes first. Announced targets are captured before the whole
                // body (see the end of `lower_scoped`), so the branch can read them.
                if target_scope
                    && let Some(TextStep::Act(clause, guarded, _)) = acts.get(index + 1)
                    && guarded.mods.iter().any(|m| matches!(m, ir::effect::Modifier::If(said) if std::ptr::eq(said, c)))
                    && said_of_named(c)
                {
                    target_branches.push((e.announced_mask, (crate::announcement::ANY, crate::announcement::ANY)));
                    let remember = e.slot();
                    let at = e.instrs.len();
                    branches.push((at, binder.clone(), remember));
                    // said in its act's sentence: what that sentence names is what it means first
                    let mut before = binder.clone();
                    before.at = (effect.0.iter().position(|s| says(s, clause)).map_or(usize::MAX, |i| base + i), index + 1);
                    deferred.push((at, c, before, e.targets.len()));
                    // read at `EndIf`: placeholder until then
                    e.instrs.push(Instr::Branch { condition: condition::Condition::stored(remember, true), end: 0, remember: Some(remember), skipped_values: Vec::new() });
                    e.end_act(false, None);
                    continue;
                }
                // An explicit attached-object noun in a condition introduces its subject
                // on both paths: "If equipped creature is a Vampire, put two on it instead".
                // Capture before branching so a skipped action cannot erase the antecedent.
                // The ability's own object named there is the antecedent of a later "it" as
                // an explicit mention in the body is ("Whenever an opponent casts a spell, if
                // this card is suspended, remove a time counter from it": from this card).
                if let ir::Condition::Is { what, .. } = c
                    && matches!(what.quant, Quant::Definite(Ref::AttachedTo(_) | Ref::This)) {
                    e.operand(what, support, &mut binder)?;
                }
                let condition = if let ir::Condition::Did { who, outcome } = c {
                    let actor = e.operand(who, support, &mut binder)?;
                    let Some((producer, paid, possible, instruction)) = &binder.outcome else { return unsupported("outcome has no earlier payment producer"); };
                    if actor != *producer { return unsupported("outcome names a different actor than its payment"); }
                    if matches!(producer, action::Operand::All(_) | action::Operand::Union(_)) { return unsupported("several-player payment needs per-player outcome predicates"); }
                    if *outcome == ir::condition::Outcome::Couldnt && matches!(e.instrs[*instruction], Instr::Pay { .. }) {
                        return unsupported("mana payment: impossibility-dependent branch needs an exhaustive payment proof");
                    }
                    e.producer(*instruction)?;
                    // what the outcome asks about is a cost (118.12), and a mill of more cards
                    // than the library has can't be paid (701.17b): nothing is milled
                    if let Instr::Action { action: action::Action::Mill { .. }, attempt: Some(attempt), .. } = &mut e.instrs[*instruction] { attempt.cost = true; }
                    condition::Condition::outcome(*paid, *possible, *outcome)
                } else if let ir::Condition::Won {player, won, contest: Some(ir::condition::Contest::Flip)} = c {
                    if !crate::select::is_you(player) { return unsupported("coin outcome: only the flipping controller's result is implemented"); }
                    let slot = binder.coin.ok_or_else(|| crate::Unsupported("coin outcome: no preceding local flip producer".into()))?;
                    condition::Condition::coin(slot, *won)
                } else if let Some(source) = countered_ability_source(c) {
                    let [(action::Operand::Reference(input), result)] = binder.countered.as_slice() else {
                        return unsupported("countered ability source: one proven counter input required");
                    };
                    if !binder.subjects.iter().any(|named| named.slot == *input && named.target
                        && named.bounds.1 <= 1 && named.set.domain == Domain::Ability) {
                        return unsupported("countered ability source: a single ability target required");
                    }
                    let filter = expr::Filter::compile(&source.filter, support, &mut binder.scope(support))?;
                    let source_operand = action::Operand::AbilitySources(Box::new(action::Operand::Reference(*result)));
                    let subjects = e.slot();
                    e.instrs.push(Instr::Capture { operand: action::Operand::Matching(action::Matching {
                        of: Box::new(source_operand), filter: Box::new(filter), controller: None }), out: subjects });
                    binder.name(subjects, source.clone(), (0, 1), false);
                    condition::Condition::present(subjects)
                } else if let Some(subject) = countered_subject(c) {
                    let subject = e.operand(subject, support, &mut binder)?;
                    let Some((_, slot)) = binder.countered.iter().rev().find(|(producer, _)| *producer == subject) else {
                        return unsupported("counter outcome: no matching earlier counter producer");
                    };
                    condition::Condition::present(*slot)
                } else { condition::Condition::compile(c, support, &mut binder.scope(support))? };
                let outcomes = if target_scope { condition.announced_outcomes() } else { (crate::announcement::ANY, crate::announcement::ANY) };
                target_branches.push((e.announced_mask, outcomes));
                e.announced_mask &= outcomes.0;
                let remember = e.slot();
                branches.push((e.instrs.len(), binder.clone(), remember));
                binder.condition_amount = condition.completed_quantity();
                e.instrs.push(Instr::Branch { condition, end: 0, remember: Some(remember), skipped_values: Vec::new() });
                e.end_act(false, None);
                continue;
            }
            TextStep::Otherwise => {
                otherwise_targets = e.targets.len();
                let (outer, outcomes) = previous_target_if.expect("a preceding target scope");
                target_branches.push((outer, outcomes));
                e.announced_mask = outer & outcomes.1;
                let Some((remember, scope)) = previous_if.take() else { return unsupported("otherwise has no preceding conditional branch"); };
                let joined = binder.completed_subject;
                binder = scope;
                let mut outgoing = binder.clone();
                outgoing.completed_subject = joined;
                branches.push((e.instrs.len(), outgoing, remember));
                e.instrs.push(Instr::Branch { condition: condition::Condition::stored(remember, false), end: 0, remember: None, skipped_values: Vec::new() });
                e.end_act(false, None);
                alternative_path(&mut binder, untaken);
                continue;
            }
            TextStep::Instead(alternative) => {
                let (outer, outcomes) = previous_target_if.expect("a preceding target scope");
                target_branches.push((outer, outcomes));
                e.announced_mask = outer & outcomes.0;
                // what the replaced sentence named ("that creature") stays named here, unless
                // its targets are the alternative to this one's, chosen only when these aren't
                let Some((remember, scope)) = previous_if.take() else { return unsupported("instead has no preceding conditional branch"); };
                // a condition not said as it is cast ("if {R}{G} was spent") has no alternative
                let alternative = alternative && outcomes != (crate::announcement::ANY, crate::announcement::ANY);
                binder = scope;
                if alternative {
                    alternative_close = Some((e.targets.len(), branches.len() + 1));
                    // the replaced sentence's targets, and the players they name, are not
                    // named here: they are not chosen when this is done
                    let replaced: Vec<Slot> = e.instrs.iter().filter_map(|i| match i {
                        Instr::Capture { operand: action::Operand::Targets(group), out } if *group >= otherwise_targets => Some(*out),
                        _ => None,
                    }).collect();
                    binder.subjects.retain(|n| !replaced.contains(&n.slot) && n.controlled_target.is_none_or(|t| !replaced.contains(&t)));
                }
                alternative_targets = alternative;
                branches.push((e.instrs.len(), binder.clone(), remember));
                e.instrs.push(Instr::Branch { condition: condition::Condition::stored(remember, true), end: 0, remember: None, skipped_values: Vec::new() });
                e.end_act(false, None);
                alternative_path(&mut binder, untaken);
                replacing = true;
                continue;
            }
            TextStep::EndIf => {
                if let Some((first, depth)) = alternative_close && branches.len() == depth {
                    // a target only in a quoted ability is not the spell's: no alternative
                    if e.targets.len() == first { return unsupported("instead: a cast-time alternative whose only target is not the spell's"); }
                    alternative_close = None;
                    alternative_targets = false;
                }
                let scope = target_branches.pop().expect("target scopes are balanced");
                e.announced_mask = scope.0;
                previous_target_if = Some(scope);
                let (at, mut outer, remember) = branches.pop().expect("text scopes are balanced");
                // a condition said after its act: what was named before it, and that act's targets
                if let Some(d) = deferred.iter().position(|(branch, ..)| *branch == at) {
                    let (_, c, mut before, first) = deferred.remove(d);
                    for n in first..e.targets.len() {
                        let captured = e.instrs.iter().find_map(|i| match i {
                            Instr::Capture { operand: action::Operand::Targets(group), out } if *group == n => Some(*out),
                            _ => None,
                        });
                        if let Some(named) = captured.and_then(|slot| binder.subjects.iter().find(|s| s.slot == slot)) {
                            before.subjects.push(named.clone());
                            // A deferred condition can also name the target's captured player.
                            before.subjects.extend(binder.subjects.iter()
                                .filter(|player| player.controlled_target == Some(named.slot)).cloned());
                        }
                    }
                    let condition = condition::Condition::compile(c, support, &mut before.scope(support))?;
                    let Instr::Branch { condition: placeholder, .. } = &mut e.instrs[at] else { unreachable!("a deferred condition's branch") };
                    *placeholder = condition;
                }
                let incoming_completed = outer.completed_subject;
                let (produced_before, damage_before) = (outer.produced.len(), outer.damage.len());
                close_branch(&mut e.instrs, at, &binder, &mut outer);
                // Announcement targets exist on either path; their captures are hoisted below.
                // Otherwise starts before the taken branch's moves: its object references
                // must not inherit that branch's entry projections (608.2c).
                let before = outer.subjects.clone();
                outer.subjects = binder.subjects.clone();
                let mut otherwise = outer.clone();
                // A direct attached mention captured inside the untaken action
                // does not exist on the otherwise/instead path. Preserve a noun
                // introduced by the condition, whose capture precedes the branch.
                otherwise.subjects.retain(|named| !named.attached);
                otherwise.subjects.extend(before.iter().filter(|named| named.attached).cloned());
                otherwise.completed_subject = incoming_completed;
                untaken = (produced_before, damage_before);
                for named in &mut otherwise.subjects {
                    named.object = before.iter().find(|n| n.slot == named.slot).and_then(|n| n.object);
                }
                previous_if = Some((remember, otherwise));
                binder = outer;
                continue;
            }
            TextStep::DuringCreatedTurn => {
                let turn = binder.created_turn.ok_or_else(||crate::Unsupported("during that turn: one preceding created turn is required".into()))?;
                created_turn_scopes.push(turn);continue;
            }
            TextStep::EndDuringCreatedTurn => { created_turn_scopes.pop();continue; }
            TextStep::AfterTurn(turn) => { after_turns.push(turn);continue; }
            TextStep::EndAfterTurn => { after_turns.pop().expect("temporal scope");continue; }
            TextStep::Duration(duration) => { durations.push(duration); continue; }
            TextStep::EndDuration => { durations.pop().expect("duration scope"); continue; }
            // "For each player, …": the players it is done for, named once. Each act in its
            // scope is one the engine does for every one of them alike, so "that player" is
            // one of them in turn and what is made is everything made for all of them. Said of
            // "each player" ("Each player searches their library …"), each of them does it.
            TextStep::Each(set, does) => {
                each_completed = Some(binder.completed_subject);
                if binder.each.is_some() || !durations.is_empty() || optional_groups != 0 { return unsupported("for each player: nested, lasting or optional"); }
                let players = e.operand(&Selector::new(Quant::All, set.clone()), support, &mut binder)?;
                let out = e.slot();
                e.instrs.push(Instr::Capture { operand: players, out });
                binder.name(out, set.clone(), (0, 1), false);
                binder.each = Some(out);
                binder.each_does = does;
                continue;
            }
            TextStep::EndEach => {
                binder.each_does = false;
                binder.completed_subject = each_completed.take().expect("for-each completed context").joined(binder.completed_subject);
                let each = binder.each.take().expect("for-each scope");
                binder.subjects.retain(|n| n.slot != each);
                // what was found for one player is, after it, what was found for them all
                if let Some((_, _, max)) = &mut binder.result { *max = u32::MAX; }
                continue;
            }
            TextStep::CastRider(stated) => {
                let types = cast_rider(stated).expect("read as a rider");
                // the cast offered just before, and not yet said of
                let Some(Instr::Cast { rider, one, .. }) = e.instrs.iter_mut().rev().find(|i| matches!(i, Instr::Cast { .. })) else {
                    return unsupported("program: a spell \"cast this way\" with no cast offered before it");
                };
                if rider.is_some() { return unsupported("program: two riders of one cast"); }
                if types.is_empty() && !*one { return unsupported("cast rider: singular spell beside a multiple-card offer"); }
                *rider = Some(crate::permission::Rider { types, by: None });
                continue;
            }
            TextStep::ExiledInstead(who) => {
                if !durations.is_empty() || optional_groups != 0 { return unsupported("program: a replacement under a duration or an option"); }
                // "If a permanent dealt damage by ~ would die this turn": of whatever this
                // spell dealt damage to, asked as each would die. Its "this turn" is the
                // replacement's (614.1a).
                if let Some(dealt) = dealt_damage_by_this(who, support) {
                    let subject = e.operand(&dealt, support, &mut binder)?;
                    e.instrs.push(Instr::Register { subject, rule: crate::restrict::Rule::exiled_instead(), lifetime: crate::restrict::Lifetime::ThisTurn });
                    e.end_act(false, None);
                    continue;
                }
                // what it names as it is put in force, and nothing that is that later (614.1a)
                let subject = e.operand(who, support, &mut binder)?;
                if matches!(subject, action::Operand::All(_)) { return unsupported("death replacement: of whatever would die, not of objects named"); }
                e.instrs.push(Instr::Register { subject, rule: crate::restrict::Rule::exiled_instead(), lifetime: crate::restrict::Lifetime::ThisTurn });
                e.end_act(false, None);
                continue;
            }
            TextStep::Rule(stated) => {
                if !durations.is_empty() || optional_groups != 0 { return unsupported("program: a rule of the game under a duration or an option"); }
                if let Some(turns) = created_turn_scopes.last() {
                    let (rule, this_turn) = crate::restrict::Rule::activation_family(stated)?;
                    if this_turn { return unsupported("created-turn rule: conflicting this-turn duration"); }
                    e.instrs.push(Instr::RuleDuringCreatedTurns { turns: *turns, rule });
                    e.end_act(false,None);continue;
                }
                // said of the game, not of a subject: its controller is who put it in force
                let (rule, lifetime) = crate::restrict::Rule::sentence(stated)?;
                e.instrs.push(Instr::Register { subject: action::Operand::Controller, rule, lifetime });
                e.end_act(false, None);
                continue;
            }
            TextStep::Alternative(clause) => {
                if !durations.is_empty() || optional_groups != 0 || binder.each.is_some() || !clause.may || !single_actor(clause.subject.as_ref()) {
                    return unsupported("alternative actions: only one player's optional object payment is implemented");
                }
                e.clause_subject = None;
                binder.clause = None;
                let actor = e.clause_subject(clause, support, &mut binder)?;
                let mut alternatives = Vec::new();
                for act in std::iter::once(&clause.first).chain(clause.rest.iter().map(|(_,act)|act)) {
                    if act.may || !act.mods.is_empty() { return unsupported("alternative actions with individual modifiers"); }
                    admitted(act)?;
                    // Unchosen branches do not introduce an antecedent into the outer scope.
                    let (action, _) = e.object_choice(act, actor.clone(), support, &mut binder.clone())?;
                    alternatives.push(action);
                }
                let action = choice::Action::alternatives(alternatives)?;
                action.payment_ok()?;
                let (paid, possible) = (e.slot(), e.slot());
                binder.outcome = Some((actor, paid, possible, e.instrs.len()));
                e.instrs.push(Instr::Choice { action, optional: true, payment: false, paid, possible, chosen: None, outcomes: None, actual: None, discarded_before: None });
                e.end_act(true, None);
                continue;
            }
            TextStep::KeywordCost(..) => return unsupported("keyword cost definition: no matching preceding grant"),
            TextStep::Act(clause, act, alone) => (clause, act, alone),
        };
        if std::ptr::eq(act, &clause.first) && heads.iter().any(|h| std::ptr::eq(*h, clause)) { e.sentence_targets = e.targets.len(); }
        if skip_rider { skip_rider = false; continue; }
        binder.past_inputs.clear();
        // "For each opponent, you may discard a card": once for each of them, which an act
        // done once doesn't say
        if binder.each.is_some() && (clause.may || act.may || !(matches!(act.action, Action::Search { .. } | Action::Exile(_) | Action::Shuffle(ir::procedure::Shuffled::Default))
            || binder.each_does && matches!(act.action, Action::Move { .. } | Action::Reveal(_)))) {
            return unsupported("for each player: an act the engine does not do for each of them alike");
        }
        let replaces = std::mem::take(&mut replacing);
        e.alternative_kicker_targets = replaces && !alternative_targets && (support.has_kicker() || support.has_bargain());
        admitted(act)?;
        let draw_bound = if let Action::DrawUpTo(Amount::Const(max)) = &act.action {
            // No No-versus-zero outcome or compound consent is represented by this policy.
            if effect.0.len() != 1 || !clause.rest.is_empty() || !alone || !act.mods.is_empty() {
                return unsupported("draw up to: standalone fixed-count action required");
            }
            Some(*max)
        } else { None };
        let definition = effect.0.iter().find_map(|sentence| clause_x(sentence, clause));
        let same_scope = matches!((x_scope, definition), (Some(previous), Some((current, _))) if std::ptr::eq(previous, current));
        if !body_x && !same_scope { x_defined = false; }
        x_scope = definition.map(|(amount, _)| amount);
        if std::ptr::eq(act, &clause.first) {
            e.clause_subject = None;
            binder.clause = None;
            let consuming = contrasts.iter().find(|(_, next, _)| std::ptr::eq(*next, clause));
            if let Some((_, _, expected)) = consuming {
                if binder.contrast.as_ref().is_none_or(|(_, set)| set != *expected) {
                    return unsupported("other: preceding group is not retained on this path");
                }
            } else { binder.contrast = None; }
            e.contrast = contrasts.iter().find(|(first, _, _)| std::ptr::eq(*first, clause)).map(|(_, _, set)| (*set).clone());
        }
        // A trailing step marker creates an ability now; its body runs at that future
        // step through this same executor, with this occurrence's bindings (603.7c).
        let at: Vec<_> = act.mods.iter().filter_map(|m| match m {
            ir::effect::Modifier::At { step, whose } => Some((*step,whose.clone())),
            ir::effect::Modifier::AtEndOfCombat => Some((ir::event::Step::EndOfCombat,ir::event::StepOwner::The { next: true })),
            _ => None,
        }).collect();
        if let Some((step,whose)) = at.first() {
            if at.len()!=1 || !clause.rest.is_empty() || !durations.is_empty() || !after_turns.is_empty() || clause.may || act.may {
                return unsupported("delayed step instruction: compound or optional scheduling");
            }
            let pattern = crate::trigger::DelayedPattern::step(*step,whose,support)?;
            let mut body = clause.clone();
            body.first.mods.retain(|m| !matches!(m,ir::effect::Modifier::At { .. } | ir::effect::Modifier::AtEndOfCombat));
            e.end_act(false,None);
            let prefix = Program::with_initial(e.instrs.clone(),e.acts.clone(),e.targets.clone(),initial.clone())?;
            let mut scope = binder.clone();
            scope.sentences = base + effect.0.len(); scope.clause = None; scope.outcome = None;
            scope.turn = None; scope.created_turn = None; scope.condition_amount = None;
            scope.searched = None; scope.found_waiting = false;
            let effect = Effect(vec![Sentence::Do(ir::effect::Body { first: body, rest: Vec::new() })]);
            // its text is the instruction with the step it is done at
            let whole = Sentence::Do(ir::effect::Body { first: clause.clone(), rest: Vec::new() });
            let program = said(lower_scoped(&effect,support,scope,prefix.kinds)?, &whole, &[]);
            e.instrs.push(Instr::Delayed { pattern, subjects: Vec::new(), program, lasts: crate::state::Lasts::Indefinite, once: true });
            e.end_act(false,None);
            continue;
        }
        e.relative_targets = matches!(act.action, Action::DealDamage(_)).then_some(e.targets.len());
        let player_control = matches!(&act.action,Action::GainControl(of) if of.items.iter().all(|s|s.set.domain==Domain::Player));
        let extra_turn = matches!(act.action,Action::Keyword(ir::KeywordAction::ExtraTurn));
        if !after_turns.is_empty() && !extra_turn { return unsupported("after turn: only extra turns are scheduled at that boundary"); }
        let copying = matches!(act.action, Action::Becomes(ir::action::Becoming::CopyOf(_)));
        let prevention = matches!(act.action, Action::Prevent(_));
        let lasting = crate::layers::changes(&act.action) || matches!(act.action, Action::GainControl(_));
        let registration = Game::keeps(&act.action) || player_grant(clause.subject.as_ref(), &act.action);
        let until_source = matches!(act.action, Action::Exile(_)) && duration_mods(clause, act, &durations).iter()
            .any(|m| matches!(m, ir::effect::Modifier::Duration(d) if crate::until::source_leaves(d)));
        // "becomes saddled until end of turn": what the designation lasts anyway (702.171b)
        let saddling = matches!(act.action, Action::Becomes(ir::action::Becoming::Saddled)) && matches!(duration_mods(clause, act, &durations).as_slice(),
            [ir::effect::Modifier::Duration(ir::effect::Duration::Until(ir::event::Time { end: true, step: ir::event::Step::Turn, whose: ir::event::StepOwner::This }))]);
        // "You may play it this turn", "Until the end of your next turn, you may play those
        // cards": permission lasting until a deadline (611.2a), not a cast done as the effect
        // resolves (which is said without a duration).
        if let Action::Cast { play, what, free: false } = &act.action
            && (clause.may || act.may) && durations.len() <= 1
            && act.mods.iter().all(|m| matches!(m, ir::effect::Modifier::Duration(_)))
            && let [ir::effect::Modifier::Duration(duration)] = duration_mods(clause, act, &durations).as_slice()
        {
            let until = crate::permission::Deadline::compile(duration)
                .ok_or_else(|| crate::Unsupported("program: a permission to play until another deadline".into()))?;
            // about the cards named before it ("that card", "those cards", "it"); "a creature
            // spell from among cards exiled with ~" describes what is cast, a rule of its own
            if what.items.iter().any(|s| matches!(s.quant, Quant::Target(_))) {
                // "You may cast target Zombie creature card from your graveyard this turn": a
                // card where it is, not one the effect exiled, which is all a permission keeps
                return unsupported("program: a permission to play a targeted card where it is");
            }
            if !what.items.iter().all(|s| matches!(s.quant, Quant::Definite(_))) {
                return unsupported("program: a permission to play cards it describes");
            }
            if !controller_only(clause.subject.as_ref()) { return unsupported("program: permission to play given to another player"); }
            let player = e.clause_subject(clause, support, &mut binder)?;
            let cards = e.objects(what, support, &mut binder)?;
            e.instrs.push(Instr::Permit { player, cards, play: *play, until });
            e.end_act(false, None);
            continue;
        }
        // "You may cast it without paying its mana cost", "you may cast it by paying {R} rather
        // than paying its mana cost" (madness): cast as the effect is applied or not at all
        // (608.2g), for that cost.
        // "You may cast an artifact, instant, or sorcery spell from your graveyard": one card of
        // the player's own graveyard the description fits, cast now for ordinary costs or
        // without its mana cost. The source zone is independent of that cost (608.2g).
        if let Action::Cast { play: false, what, free } = &act.action
            && let [s] = what.items.as_slice()
            && matches!(s.quant, Quant::Indefinite(Count::Exactly(Amount::Const(1))) | Quant::Target(Count::Exactly(Amount::Const(1))))
            && matches!(s.set.domain, Domain::Spell | Domain::Card)
            && s.set.zone.as_ref().is_some_and(|z| z.zones.items == [ir::Zone::Graveyard] && z.slice.is_none() && z.owner.as_deref().is_some_and(crate::select::is_you))
        {
            if !(clause.may || act.may) || !durations.is_empty() || !act.mods.is_empty() || !controller_only(clause.subject.as_ref())
                || s.chooser.is_some() || s.random.is_some() || s.set.controller.is_some() || s.set.owner.is_some() || s.set.caster.is_some() {
                return unsupported("program: a cast from a graveyard other than its controller's own optional one");
            }
            let player = e.clause_subject(clause, support, &mut binder)?;
            // a card that would be such a spell: its card characteristics (601.3e)
            let set = ObjectSet { domain: Domain::Card, ..s.set.clone() };
            let cards = if matches!(s.quant, Quant::Target(_)) { vec![e.object(s, support, &mut binder)?] }
                else { vec![action::Operand::Zone(expr::ZoneCards::compile(&set, support, &mut binder.scope(support))?)] };
            let (offered, cast, possible) = (e.slot(), e.slot(), e.slot());
            binder.outcome = Some((player.clone(), cast, possible, e.instrs.len()));
            e.instrs.push(Instr::Cast { player, cards, offered,
                costs: if *free { crate::permission::Costs::Free } else { crate::permission::Costs::Ordinary },
                cast, possible, from: CastOrigin::Fixed(Zone::Graveyard), rider: None, haste: false, one: true, ceiling: None });
            e.end_act(false, None);
            continue;
        }
        if let Action::Cast { play: false, what, free } = &act.action {
            use crate::permission::Costs;
            let costs = match (free, act.mods.as_slice()) {
                (true, []) => Some(Costs::Free),
                (false, [ir::effect::Modifier::By(said)]) if said.kind == ir::statics::CastCostKind::Alternative => {
                    // Only the price linked to this very keyword-generated casting trigger.
                    let row = support.linked_cost_offer(&crate::keyword::said_cost(said))
                        .ok_or_else(|| crate::Unsupported("program: a cast for an alternative cost an effect states".into()))?;
                    Some(Costs::Only(crate::announcement::CastCost::Alternative(row)))
                }
                _ => None,
            };
            if let Some(costs) = costs && (clause.may || act.may) && durations.is_empty() {
                let card_owner = matches!(clause.subject.as_ref(), Some(Subject::Selector(s))
                    if s.set == ObjectSet::of(Domain::Player) && s.chooser.is_none() && s.random.is_none()
                    && matches!(&s.quant, Quant::Definite(Ref::OwnerOf(of))
                        if of.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::TheParticiple { what: Verb::Exile, plural: false }))
                            && of.set == ObjectSet::of(Domain::Card) && of.chooser.is_none() && of.random.is_none()));
                if !controller_only(clause.subject.as_ref()) && !card_owner {
                    return unsupported("program: a cast offered to another player");
                }
                // A completed-action card predicate must not silently become a
                // literal lexical target. Only the owning plain exile-result path
                // below is modeled; other verbs/constraints remain explicit refusals.
                fn completed_card_result(filter: &ir::Filter) -> bool {
                    match filter {
                        ir::Filter::Atom(Atom::Reference(r)) => r.expected == Kind::Cards && r.form == RefForm::ActionResult,
                        ir::Filter::All(parts) | ir::Filter::Any(parts) => parts.iter().any(completed_card_result),
                        ir::Filter::Not(inner) => completed_card_result(inner),
                        _ => false,
                    }
                }
                let exile_result = ObjectSet { filter: ir::Filter::All(vec![ir::Filter::Atom(Atom::Reference(UnresolvedRef {
                    expected: Kind::Cards, form: RefForm::ActionResult, qualifiers: vec![Qualifier::ThisWay(Verb::Exile)] }))]),
                    ..ObjectSet::of(Domain::Card) };
                if what.items.iter().any(|s| completed_card_result(&s.set.filter)
                    && !(matches!(s.quant, Quant::Definite(Ref::Anaphor(ir::Anaphor::The { .. }))) && s.set == exile_result)) {
                    return unsupported("cast: completed-card result predicate not modeled");
                }
                let player = e.clause_subject(clause, support, &mut binder)?;
                // "the copies": what the text's copy instruction made, each offered on its
                // own (707.12a)
                let cards = match what.items.as_slice() {
                    [s] if matches!(s.quant, Quant::Definite(Ref::Anaphor(ir::Anaphor::Copy { .. } | ir::Anaphor::ThatCopy))) => {
                        vec![action::Operand::Reference(binder.copies.ok_or_else(|| crate::Unsupported("reference: no copy made before it".into()))?)]
                    }
                    _ => e.objects(what, support, &mut binder)?,
                };
                let cards = cards.into_iter().map(|card| {
                    // A completed self-exile supplies its actual successor, not the
                    // original source incarnation (400.7j). Do not reacquire it later.
                    if card == action::Operand::Source {
                        if let Some(out) = e.instrs.iter().rev().find_map(|instruction| match instruction {
                            Instr::Action { action: action::Action::Move { groups, .. }, created, .. }
                                if groups.iter().any(|(objects, _)| objects.contains(&action::Operand::Source)) => Some(
                                    match groups.as_slice() {
                                        [(objects, action::Destination::Zone(Zone::Exile))] if objects == &[action::Operand::Source] => *created,
                                        _ => None,
                                    }),
                            _ => None,
                        }).flatten() {
                            let subjects = e.slot();
                            e.instrs.push(Instr::Subjects { cards: out, out: subjects, still_in: None });
                            return action::Operand::Reference(subjects);
                        }
                    }
                    card
                }).collect::<Vec<_>>();
                let from = if matches!(costs, Costs::Only(_)) {
                    // The owning linked keyword proves its exact source zone.
                    CastOrigin::Fixed(if support.generated_by("Miracle") { Zone::Hand } else { Zone::Exile })
                } else {
                    let mut origins = cards.iter().map(|card| {
                        if *card == action::Operand::Source {
                            if support.source_scope() == Some(crate::functions::Scope::In(Zone::Exile)) { return Some(CastOrigin::Fixed(Zone::Exile)); }
                        }
                        e.cast_origin(card)
                    });
                    let from = origins.next().flatten().ok_or_else(|| crate::Unsupported("cast: no fixed source zone supplied by its card producer".into()))?;
                    if !origins.all(|origin| origin == Some(from)) {
                        return unsupported("cast: cards from mixed or unproved source zones");
                    }
                    from
                };
                let (offered, cast, possible) = (e.slot(), e.slot(), e.slot());
                // "If you don't, …": whether a cast was made (601.2h)
                binder.outcome = Some((player.clone(), cast, possible, e.instrs.len()));
                let ceiling = if support.generated_by("Cascade") {
                    let scan = e.instrs.iter().rev().find_map(|instruction| match instruction {
                        Instr::LibraryScan { hit, bound, .. } => Some((*hit, *bound)), _ => None,
                    }).ok_or_else(|| crate::Unsupported("cascade: a retained scan must precede its cast offer".into()))?;
                    let supplied_hit = cards.iter().all(|card| matches!(card, action::Operand::Reference(subjects)
                        if e.instrs.iter().any(|instruction| matches!(instruction,
                            Instr::Subjects { cards, out, .. } if *cards == scan.0 && out == subjects))));
                    if !supplied_hit { return unsupported("cascade: only its exact scan hit is offered"); }
                    Some(scan.1)
                } else { None };
                e.instrs.push(Instr::Cast { player, cards, offered, costs, cast, possible, from, rider: None, haste: support.suspend_offer(), one: false, ceiling });
                e.end_act(false, None);
                continue;
            }
        }
        // "Copy it three times": copies of a card the text chose, made where the card is to
        // be cast as the effect is applied (707.12). Copying a spell is another instruction.
        if let Action::Copy(what) = &act.action
            && let [selector] = what.items.as_slice()
            && matches!(selector.quant, Quant::Definite(Ref::Anaphor(_)))
            && binder.subject(selector).is_ok_and(|slots| !slots.is_empty()
                && slots.iter().all(|slot| binder.subjects.iter().any(|n| n.slot == *slot && n.set.domain == Domain::Card)
                    || binder.result.as_ref().is_some_and(|(result, set, _)| result == slot && set.domain == Domain::Card)))
        {
            if clause.may || act.may || !durations.is_empty() || !controller_only(clause.subject.as_ref()) { return Err(not_lowered(&act.action)); }
            let times = match act.mods.as_slice() {
                [] => expr::Amount::constant(1),
                [ir::effect::Modifier::Times(n)] => quantity(n, support, &binder)?,
                [modifier, ..] => return unsupported(format!("modifier: {} of Copy", crate::select::name(modifier))),
            };
            let cards = e.operand(selector, support, &mut binder)?;
            let out = e.slot();
            e.instrs.push(Instr::CopyCards { cards, times, out });
            binder.copies = Some(out);
            e.end_act(false, None);
            continue;
        }
        if let Action::Copy(what) = &act.action {
            if clause.may || act.may || !durations.is_empty() || !controller_only(clause.subject.as_ref()) { return Err(not_lowered(&act.action)); }
            fn stack(set: &ObjectSet) -> bool { matches!(set.domain, Domain::Spell | Domain::Ability) }
            if what.conj != ir::Conj::And && what.items.len() > 1 {
                return unsupported("choice between object groups not lowered");
            }
            if !what.items.iter().all(|s| stack(&s.set) || input::stack_reference(support, s).is_some()) { return unsupported("copy: only stack spells or abilities (707.10)"); }
            let times = match act.mods.as_slice() {
                [] => expr::Amount::constant(1),
                [ir::Modifier::Times(n) | ir::Modifier::ForEach(n)] => quantity(n, support, &binder)?,
                _ => return unsupported("spell copy modifier"),
            };
            let cards = action::Operand::Union(what.items.iter().map(|selector| {
                if let Some(input) = input::stack_reference(support, selector)
                    && (binder.subjects.len(), binder.source_named) == inherited_antecedents
                {
                    let out = e.slot();
                    e.instrs.push(Instr::Input { input, out });
                    Ok(action::Operand::Reference(out))
                } else if stack(&selector.set) { e.object(selector, support, &mut binder) }
                else { unsupported("copy: delayed-body antecedent is not proved a stack participant") }
            }).collect::<Res<Vec<_>>>()?);
            let out = e.slot();
            e.instrs.push(Instr::CopyStack { cards, times, retarget: false, out });
            binder.copies = Some(out);
            e.end_act(false, None);
            continue;
        }
        let existing_targets = match &act.action {
            Action::ChangeTarget(objects) if !(clause.may || act.may) => Some((objects, crate::state::TargetPolicy::RequiredSingle)),
            Action::ChooseNewTargets { for_, one: false } if (clause.may || act.may)
                && !matches!(for_.items.as_slice(), [s] if matches!(s.quant, Quant::Definite(Ref::Anaphor(ir::Anaphor::Copy { .. } | ir::Anaphor::ThatCopy)))) => Some((for_, crate::state::TargetPolicy::ChooseNew)),
            _ => None,
        };
        if let Some((objects, policy)) = existing_targets {
            fn stack(set: &ObjectSet) -> bool { match &set.domain {
                Domain::Spell | Domain::Ability => true,
                Domain::Union(arms) => !arms.is_empty() && arms.iter().all(stack),
                _ => false,
            } }
            fn single_filter(filter: &ir::Filter) -> bool { match filter {
                ir::Filter::Atom(ir::Atom::TargetCount(Count::Exactly(Amount::Const(1)) | Count::Precisely(Amount::Const(1)))) => true,
                ir::Filter::All(parts) => parts.iter().any(single_filter),
                _ => false,
            } }
            fn single(set: &ObjectSet) -> bool { single_filter(&set.filter)
                || matches!(&set.domain, Domain::Union(arms) if !arms.is_empty() && arms.iter().all(single)) }
            let [selector] = objects.items.as_slice() else { return unsupported("target change: one stack object"); };
            if !controller_only(clause.subject.as_ref()) || !act.mods.is_empty() || !durations.is_empty()
                || !matches!(selector.quant, Quant::Target(Count::Exactly(Amount::Const(1)))) || !stack(&selector.set)
                || policy == crate::state::TargetPolicy::RequiredSingle && !single(&selector.set) {
                return unsupported("target change: one selected stack object, required single or optional new targets");
            }
            let object=e.object(selector,support,&mut binder)?;
            e.instrs.push(Instr::ChangeTargets { object, policy });
            e.end_act(false,None);continue;
        }
        if let Action::ChooseNewTargets { for_, one: false } = &act.action {
            if !(clause.may || act.may) || !act.mods.is_empty() || !controller_only(clause.subject.as_ref())
                || !matches!(for_.items.as_slice(), [s] if matches!(s.quant, Quant::Definite(Ref::Anaphor(ir::Anaphor::Copy { .. } | ir::Anaphor::ThatCopy)))) {
                return unsupported("new targets: optional targets of the preceding spell copies");
            }
            let Some(Instr::CopyStack { retarget, out, .. }) = e.instrs.last_mut() else { return unsupported("no preceding stack copies"); };
            if binder.copies != Some(*out) { return unsupported("new targets refer to other copies"); }
            *retarget = true;
            e.end_act(false, None);
            continue;
        }
        if let Action::Static(s) = &act.action && let ir::StaticAct::Skip(ir::statics::Skipped::Turns(count)) = &**s {
            if clause.may || act.may || !durations.is_empty() || !act.mods.is_empty() || binder.each.is_some() {
                return unsupported("whole-turn skip: one plain next-turn count without duration or optional payment");
            }
            if acts[index+1..].iter().any(|step| !matches!(step,TextStep::EndIf|TextStep::EndDuration|TextStep::EndMay)) {
                return unsupported("whole-turn skip followed by another action (614.10b)");
            }
            let players = e.clause_subject(clause,support,&mut binder)?;
            let count = quantity(count,support,&binder)?;
            e.instrs.push(Instr::SkipTurns {players,count});
            e.end_act(false,None);continue;
        }
        // "Target player skips their next draw step", "…their next combat phase this turn":
        // a skip effect that waits for the occurrence (614.10)
        if let Action::Static(s) = &act.action && let ir::StaticAct::Skip(ir::statics::Skipped::Step { step, next: true }) = &**s {
            use crate::turn::Occurrence;
            let what = match step {
                ir::statics::SkipStep::Untap => Occurrence::Part(ir::Part::Untap),
                ir::statics::SkipStep::Upkeep => Occurrence::Part(ir::Part::Upkeep),
                ir::statics::SkipStep::Draw => Occurrence::Part(ir::Part::Draw),
                ir::statics::SkipStep::Combat => Occurrence::Phase(ir::turn::Phase::Combat),
            };
            let this_turn = match act.mods.as_slice() {
                [] => false,
                [ir::effect::Modifier::Duration(ir::effect::Duration::ThisTurn)] if what == Occurrence::Phase(ir::turn::Phase::Combat) => true,
                _ => return unsupported("skip: only the next step or phase, or the next combat phase this turn"),
            };
            if clause.may || act.may || !durations.is_empty() { return unsupported("skip: optional or with a duration"); }
            let players = e.clause_subject(clause, support, &mut binder)?;
            e.instrs.push(Instr::Skip { players, what, this_turn });
            e.end_act(false, None);
            continue;
        }
        // "After this main phase, there is an additional combat phase followed by an
        // additional main phase" (500.8): every main phase after the first is a postcombat
        // main phase (505.1a)
        if let Action::AdditionalPhase { after, kind, count, then_main } = &act.action {
            use ir::turn::Phase;
            if !act.mods.is_empty() || clause.may || act.may || !durations.is_empty() { return unsupported("additional phase: modified or optional"); }
            let kind = if kind.main() { Phase::PostcombatMain } else { *kind };
            let mut kinds = vec![kind];
            if *then_main { kinds.push(Phase::PostcombatMain); }
            let count = quantity(count, support, &binder)?;
            e.instrs.push(Instr::AddPhases { after: *after, kinds, count });
            e.end_act(false, None);
            continue;
        }
        if !prevention && !lasting && !copying && !registration && !until_source && !saddling && !durations.is_empty() { return unsupported("program: duration on a non-continuous instruction"); }
        let mut x = None;
        let mut each = None;
        let mut unless = None;
        let mut during_turn = None;
        let mut after_turn = after_turns.last().copied();
        let mut turn_count = None;
        let mut face_down = false;
        for modifier in &act.mods {
            match modifier {
                ir::effect::Modifier::FaceDown if matches!(act.action, Action::Exile(_)) && !face_down => face_down = true,
                ir::effect::Modifier::During(during) if player_control && during_turn.is_none() => during_turn=Some(during),
                ir::effect::Modifier::AfterTurn(turn) if extra_turn && after_turn.is_none() => after_turn=Some(*turn),
                ir::effect::Modifier::Times(count) if extra_turn && turn_count.is_none() => turn_count=Some(count),
                ir::effect::Modifier::If(_) => {},
                ir::effect::Modifier::Except(_) if matches!(act.action, Action::CreateCopy { .. }) => {},
                ir::effect::Modifier::Unless { who, actions } if unless.is_none() => {
                    if actions.conj != ir::Conj::And || actions.items.is_empty() { return unsupported("unless: alternative payments"); }
                    unless = Some((who.clone(), Unless::compile(&actions.items)?));
                },
                ir::effect::Modifier::Instead if replaces => {},
                ir::effect::Modifier::Duration(d) if prevention || lasting || copying || registration || saddling || (until_source && crate::until::source_leaves(d)) => {},
                ir::effect::Modifier::WhereX(n) if x.is_none() => x = Some(n),
                // "investigate twice": the keyword action done that many times (701.16a)
                ir::effect::Modifier::Times(Amount::Const(_)) if matches!(act.action, Action::Keyword(ir::KeywordAction::Investigate | ir::KeywordAction::ManifestDread)) => {},
                ir::effect::Modifier::ForEach(n) if each.is_none() && (lasting || matches!(act.action, Action::Counter(_) | Action::PayMana(_) | Action::AddMana(_) | Action::AddAdditionalMana(_) | Action::Draw(_) | Action::DrawAdditional(_) | Action::Mill(_) | Action::GainLife(_) | Action::LoseLife(_) | Action::DealDamage(_) | Action::PutCounters(_) | Action::GetCounters { .. } | Action::Create(_))) => each = Some(n),
                // a modifier the engine does not follow is refused with the act it is said of
                _ => return unsupported(format!("modifier: {} of {}", crate::select::name(modifier), crate::select::name(&act.action))),
            }
        }
        if matches!(act.action, Action::Counter(_)) && let Some(n) = each.take() {
            let Some((_, Unless::Cost { each, .. })) = &mut unless else {
                return unsupported("for-each counter: a generic unless-payment required");
            };
            *each = Some(n.clone());
        }
        e.unless_said = unless;
        // Counts needed during announcement use the same declared expression, before costs.
        e.target_x = definition.map(|(amount, _)| amount.clone());
        // What this coordinated instruction says X is, until its first use.
        let stated = if x_defined { None } else { definition.map(|(amount, _)| amount) };
        binder.x = stated.map(|_| std::cell::Cell::new(false));
        let scoped_support = support.clone().with_x(support.x_available() || x_defined || stated.is_some());
        let support = &scoped_support;
        let mut moves = false;
        let optional = draw_bound.is_some() || (clause.may && optional_groups == 0) || act.may;
        let result_move = matches!(&act.action, Action::Move { moves, .. } if moves.iter().any(|(objects, _)| objects.items.iter().any(|s| mention(s).is_some())));
        // "You may exile it": of objects already named, an optional payment like a discard
        // (118.12), offered only where it can be done and answered by whether it was begun
        let named_exile = optional && matches!(&act.action, Action::Exile(objects)
            if objects.items.iter().all(|s| matches!(s.quant, Quant::Definite(Ref::This | Ref::Anaphor(_)))));
        // "exile … face down" (406.3) is followed of the top cards of a library and of one of
        // the cards a player just looked at; see the two exile instructions below
        if face_down && (until_source || named_exile
            || !matches!(&act.action, Action::Exile(objects) if matches!(objects.items.as_slice(), [s] if mention(s).is_some() || top_of_library(s).is_some())))
        {
            return unsupported("exile face down: other than the top of a library or a card looked at");
        }
        let causative_loss = matches!(&act.action, Action::Have { action, .. } if matches!(**action,Action::LoseLife(_)));
        if causative_loss && !controller_only(clause.subject.as_ref()) { return unsupported("causative life loss: one controller chooses"); }
        let ordinary_attempt = causative_loss || (optional && matches!(act.action, Action::Destroy(_))) || matches!(&act.action, Action::RemoveCounters { .. } | Action::PayEnergy(_) | Action::Draw(_) | Action::DrawAdditional(_) | Action::DrawUpTo(_) | Action::Mill(_) | Action::GainLife(_) | Action::LoseLife(_) | Action::Tap(_) | Action::Untap(_) | Action::PutCounters(_) | Action::GetCounters { .. } | Action::Create(_) | Action::GetEmblem(_) | Action::Move { .. } | Action::Exile(_)) && !result_move;
        let optional_top = optional && matches!(&act.action, Action::Reveal(o) if o.items.iter().all(|s| top_of_library(s).is_some()));
        let flash_permission = matches!(&act.action, Action::Static(s) if matches!(**s, ir::statics::StaticAct::CastAsThoughFlash { .. }));
        let optional_branch = if optional && !flash_permission && !optional_top && !matches!(act.action, Action::Scry(_) | Action::Surveil(_)) && !ordinary_attempt && !result_move && !named_exile && !matches!(act.action, Action::Discard(_) | Action::Sacrifice(_) | Action::PayMana(_) | Action::PayLife(_) | Action::Numbered(ir::NumberedAction::CollectEvidence, _) | Action::OnObjects(ir::action::ObjectAction::Behold, _)) {
            if !alone || !controller_only(clause.subject.as_ref()) {
                return unsupported("program: optional compound clause or other chooser not lowered");
            }
            let out = e.slot();
            e.instrs.push(Instr::Gate { who: Who::Controller, probe: None, out, possible: None });
            let branch = e.instrs.len();
            e.instrs.push(Instr::SkipUnless { consent: out, count: 0 });
            Some(branch)
        } else { None };
        if stated.is_some() || x_defined {
            // Discover's defining value refers to its input, not the card its scan finds.
            let before = if matches!(act.action, Action::Numbered(ir::NumberedAction::Discover, _)) {
                stated.map(|n| quantity(n, support, &binder)).transpose()?
            } else { None };
            let end = definition.map(|(_, end)| end);
            let last = end.is_none_or(|end| std::ptr::eq(act, end));
            // A local gate may skip SetX while a later grammatical subject still runs.
            // Ordinary optional actions put their choice after SetX and are safe; a
            // sentence-level lead gates every consumer together. Refuse the remaining
            // cross-branch lifetime rather than reading an uninitialized base X.
            let branch_local = optional_branch.is_some()
                || act.mods.iter().any(|m| matches!(m, ir::Modifier::If(_)))
                || clause.may && !alone && end.is_some_and(|end|
                    !std::ptr::eq(end, clause.rest.last().map_or(&clause.first, |(_, a)| a)));
            defining = Some(WhereX { stated, start: e.instrs.len(), gated: optional_branch.is_some(), branch_local, last, before });
        }
        if matches!(act.action, Action::Draw(_) | Action::DrawAdditional(_) | Action::DrawUpTo(_) | Action::Mill(_) | Action::GainLife(_) | Action::LoseLife(_) | Action::GetCounters { .. }) {
            let players = e.clause_subject(clause, support, &mut binder)?;
            if matches!(players, action::Operand::All(_) | action::Operand::Union(_)) {
                binder.clause = Some(expr::Subject::Actor);
            }
        }
        if registration {
            let subject = e.clause_subject(clause, support, &mut binder)?;
            let chosen = choose_protection_color(&mut e, &mut binder, &act.action);
            let rule = crate::restrict::Rule::compile(chosen.as_ref().unwrap_or(&act.action), support, &mut binder.scope(support))?;
            rule.validate_entry_subject(None)?;
            let mods = duration_mods(clause, act, &durations);
            let lifetime = crate::restrict::Lifetime::compile(&mods, &act.action)?;
            e.instrs.push(Instr::Register { subject, rule, lifetime });
            e.end_act(false, optional_branch);
            continue;
        }
        if extra_turn {
            if each.is_some() || after_turns.len()>1 { return unsupported("extra turns: explicit count and one temporal boundary"); }
            let players=e.clause_subject(clause,support,&mut binder)?;
            let after=match after_turn {
                Some(ir::effect::TurnRef::This) | None => None,
                Some(ir::effect::TurnRef::That) => Some(binder.turn.ok_or_else(||crate::Unsupported("that turn: no preceding future-turn anchor".into()))?),
            };
            let count=turn_count.map(|n|quantity(n,support,&binder)).transpose()?.unwrap_or_else(||expr::Amount::constant(1));
            let out=e.slot();
            binder.created_turn = (turn_count.is_none_or(|n| matches!(n,Amount::Const(1))) && single_actor(clause.subject.as_ref())).then_some(out);
            e.instrs.push(Instr::ExtraTurns {players,count,after,out});
            e.end_act(false,optional_branch);continue;
        }
        if player_control {
            let Action::GainControl(of)=&act.action else { unreachable!() };
            let [selected]=of.items.as_slice() else { return unsupported("player control: one selected player"); };
            if !controller_only(clause.subject.as_ref()) || each.is_some() || !durations.is_empty()
                || !matches!(selected.quant,Quant::Target(Count::Exactly(Amount::Const(1)))) {
                return unsupported("player control: controller chooses one target player's next turn");
            }
            let Some(ir::event::During::Step(ir::event::Step::Turn,ir::event::StepOwner::Player {who,next:true}))=during_turn else {
                return unsupported("player control: during that player's next turn");
            };
            let player=e.operand(selected,support,&mut binder)?;
            if e.operand(who,support,&mut binder)? != player {
                return unsupported("player control: duration must name the controlled player's next turn");
            }
            let out=e.slot();binder.turn=Some(out);
            e.instrs.push(Instr::ControlTurn {player,out});
            e.end_act(false,optional_branch);continue;
        }
        if let Action::GainControl(of) = &act.action {
            if of.items.iter().any(|s| !s.set.possibilities().iter().all(|p| p.domain.sort.is_a(ir::Sort::Permanent))
                || s.set.zone.as_ref().is_some_and(|z| z.zones.items != [ir::Zone::Battlefield])) {
                return unsupported("gain control: only permanents, not spell occurrences");
            }
            if !controller_only(clause.subject.as_ref()) || each.is_some() {
                return unsupported("gain control: a different player or a repeated quantity");
            }
            let objects = e.objects(of, support, &mut binder)?;
            let mods = duration_mods(clause, act, &durations);
            let duration = change::Duration::compile(&mods)?;
            e.instrs.push(Instr::Continuous { objects, changes: vec![change::Change::Controller], each: None, duration });
            e.end_act(false, optional_branch);
            continue;
        }
        if let Action::Becomes(ir::action::Becoming::CopyOf(from)) = &act.action {
            // This slice copies one targeted permanent/card onto battlefield permanents.
            // Spell copies, static copying and exceptions have different rules (707.9–10).
            if !matches!(from.quant, Quant::Target(ir::Count::Exactly(Amount::Const(1))))
                || !from.set.possibilities().iter().all(|p| p.domain.sort.is_a(ir::Sort::Permanent) || p.domain.sort == ir::Sort::Card)
                || from.set.zone.as_ref().is_some_and(|z| z.zones.items.contains(&ir::Zone::Stack)) {
                return unsupported("resolved copy: one targeted permanent or nonstack card");
            }
            let Some(Subject::Selector(subject) | Subject::Each(subject)) = &clause.subject else {
                return unsupported("resolved copy: battlefield permanent subjects");
            };
            if !subject.set.possibilities().iter().all(|p| p.domain.sort.is_a(ir::Sort::Permanent))
                || subject.set.zone.as_ref().is_some_and(|z| z.zones.items != [ir::Zone::Battlefield]) || each.is_some() {
                return unsupported("resolved copy: battlefield permanent subjects");
            }
            if e.clause_subject.is_none() {
                let operand = e.clause_subject(clause, support, &mut binder)?;
                let out = e.slot();
                if operand == action::Operand::Source { e.captured_source = Some(out); }
                e.instrs.push(Instr::Capture { operand, out });
                e.clause_subject = Some((action::Operand::Reference(out), Ok(())));
            }
            let objects = vec![e.clause_subject(clause, support, &mut binder)?];
            let from = e.operand(from, support, &mut binder)?;
            let duration = change::Duration::compile(&duration_mods(clause, act, &durations))?;
            let turn = match duration {
                change::Duration::Turn => true, change::Duration::Forever => false,
                change::Duration::SourceControlled | change::Duration::SourcePresent => return unsupported("resolved copy: conditional duration"),
            };
            e.instrs.push(Instr::Copy { objects, from, turn });
            e.end_act(false, optional_branch);
            continue;
        }
        if lasting {
            fn object_subject(subject: &Subject) -> bool {
                let set = match subject {
                    Subject::Selector(s) | Subject::Each(s) => &s.set,
                    Subject::Set(set) => set,
                    Subject::And { items, .. } => return items.iter().all(object_subject),
                    Subject::LifeTotal(_) => return false,
                };
                !set.possibilities().iter().any(|p| p.domain.sort == ir::Sort::Player)
            }
            let Some(subject) = &clause.subject else { return unsupported("continuous effect without a subject"); };
            crate::layers::removal_scope(&act.action, subject)?;
            // "Put a +1/+1 counter on each legendary creature you control. They gain
            // vigilance": a bare "they" is read as players, and is the permanents the text
            // named where neither it nor the event named players it could be.
            let named = match subject {
                Subject::Selector(s) if s.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::They)) && kinds(&s.set) == (true, false)
                    && matches!(binder.subject(s).as_deref(), Ok([]))
                    && matches!(input::find(support, s), Ok(found) if found.is_none_or(|input| input::own(support, input))) => {
                    let objects = Selector { set: ObjectSet::of(ir::Domain::Permanent), ..s.clone() };
                    binder.names_objects(&objects).then_some(objects)
                }
                _ => None,
            };
            if !object_subject(subject) && named.is_none() { return unsupported("continuous changes to players are not lowered"); }
            // 611.2c: the subjects of this effect are fixed when it begins, including
            // across coordinated changes that themselves change which objects fit.
            if e.clause_subject.is_none() {
                let operand = match named {
                    Some(objects) => e.operand(&objects, support, &mut binder)?,
                    None => e.clause_subject(clause, support, &mut binder)?,
                };
                let out = e.slot();
                // "Creatures you control get +2/+0 … Untap those creatures."
                if let (action::Operand::All(_), Subject::Selector(Selector { set, .. }) | Subject::Each(Selector { set, .. }) | Subject::Set(set)) = (&operand, subject) {
                    binder.name(out, set.clone(), (0, u32::MAX), false);
                }
                if operand == action::Operand::Source { e.captured_source = Some(out); }
                e.instrs.push(Instr::Capture { operand, out });
                e.clause_subject = Some((action::Operand::Reference(out), Ok(())));
            }
            let objects = vec![e.clause_subject(clause, support, &mut binder)?];
            let mut chosen = choose_protection_color(&mut e, &mut binder, &act.action);
            // what gives itself an ability: its name there is it (201.5a)
            if e.captured_source.is_some_and(|out| objects == [action::Operand::Reference(out)])
                && let Some(own) = granted_to_itself(chosen.as_ref().unwrap_or(&act.action)) { chosen = Some(own); }
            let mut changes = if let Some(TextStep::KeywordCost(keyword, of)) = acts.get(index + 1) {
                // The bare possessive in a keyword's adjacent definition names that
                // grant's recipient, distributively for an Each/All subject (611.2c).
                // An explicit different possessor or an unpaired cost remains refused.
                let own = (of.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::It)) && of.set == ObjectSet::of(Domain::Object))
                    || (of.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::That)) && of.set == ObjectSet::of(Domain::Card)
                        && matches!(subject, Subject::Selector(s) if s.set.domain == Domain::Card
                            && s.quant == Quant::Target(Count::Exactly(Amount::Const(1)))));
                if !matches!(&act.action, Action::Gains(granted) if matches!(granted.as_slice(), [ir::action::Granted::Keyword(item)]
                    if item.keyword == *keyword && item.param == ir::token::KeywordParam::None))
                    || !own
                    || of.chooser.is_some() || of.random.is_some() || keyword.name() != "Flashback" {
                    return unsupported("keyword cost definition: only the granted flashback's own mana cost is implemented");
                }
                reach.skip_through = index + 2;
                vec![change::Change::Fixed(crate::layers::Change::Gains(vec![crate::keyword::Held::flashback_at_mana_cost()]))]
            } else { change::Change::compile(chosen.as_ref().unwrap_or(&act.action), support, &mut binder.scope(support))? };
            if changes.iter().any(|change| matches!(change, change::Change::Fixed(crate::layers::Change::Inherits(_)))) {
                return unsupported("inherited abilities: only continuous battlefield static acquisition is modeled");
            }
            if matches!(acts[index + 1..].iter().find(|step| !matches!(step, TextStep::EndDuration | TextStep::EndIf)), Some(TextStep::Act(c, a, _))
                if c.rest.is_empty() && !c.may && !a.may && a.mods.is_empty()
                    && matches!(c.subject.as_ref(), Some(Subject::Selector(Selector { quant: Quant::Definite(Ref::This | Ref::Anaphor(_)), .. })))
                    && matches!(&a.action, Action::Becomes(ir::action::Becoming::Still(k)) | Action::Is(ir::action::Becoming::Still(k)) if k.subtypes.is_empty())
                    && matches!(act.action, Action::Becomes(_))) {
                changes = change::Change::still(changes);
                skip_rider = true;
            }
            let mods = duration_mods(clause, act, &durations);
            let duration = change::Duration::compile(&mods)?;
            // "it gets +1/+1 for each other creature you control": other than what gets it, one
            // object, not the source of the effect (Sigil of Valor's equipped creature)
            let one = matches!(subject, Subject::Selector(s) if matches!(s.quant, Quant::Definite(Ref::Anaphor(ir::Anaphor::It) | Ref::AttachedTo(_))));
            let mut context = binder.recipients.clone();
            if let (true, [action::Operand::Reference(slot)]) = (one, objects.as_slice()) {
                context.subject = Some(expr::Subject::Local(*slot));
            }
            let each = binder.with_recipients(context, |binder| each.map(|n| quantity(n, support, binder)).transpose())?;
            e.instrs.push(Instr::Continuous { objects, changes, each, duration });
            e.end_act(false, optional_branch);
            continue;
        }
        if act.action == Action::Keyword(ir::KeywordAction::FlipCoin) {
            if optional || !branches.is_empty() || binder.actor.is_some() || binder.each.is_some()
                || !controller_only(clause.subject.as_ref()) || !act.mods.is_empty() {
                return unsupported("coin flip: only an unconditional single called flip by the controller is implemented");
            }
            let out = e.slot();
            binder.coin = Some(out);
            e.instrs.push(Instr::FlipCoin {out});
            e.end_act(false, None);
            continue;
        }
        // 701.57a-c: sequential scan, ordinary cast/hand route and random-bottom
        // continuation remain one resolving action, without intermediate SBAs.
        if let Action::Numbered(ir::NumberedAction::Discover, n) = &act.action {
            if optional || !single_actor(clause.subject.as_ref()) || binder.each.is_some()
                || act.mods.iter().any(|m| !matches!(m, ir::effect::Modifier::WhereX(_))) {
                return unsupported("discover: mandatory single-player action without other modifiers required");
            }
            let amount = quantity(n, support, &binder)?;
            let player = e.clause_subject(clause, support, &mut binder)?;
            let actor = e.slot();
            e.instrs.push(Instr::Capture { operand: player.clone(), out: actor });
            let by = Who::Of(actor);
            let (all, hit, designated, bound, subjects, offered, cast, possible, rest, designated_subjects) =
                (e.slot(), e.slot(), e.slot(), e.slot(), e.slot(), e.slot(), e.slot(), e.slot(), e.slot(), e.slot());
            e.instrs.push(Instr::LibraryScan { by, amount, limit: ScanLimit::AtMost, all, hit, designated, bound });
            e.instrs.push(Instr::Subjects { cards: hit, out: subjects, still_in: Some(Zone::Exile) });
            e.instrs.push(Instr::Subjects { cards: designated, out: designated_subjects, still_in: None });
            e.instrs.push(Instr::Cast { player, cards: vec![action::Operand::Reference(subjects)],
                offered, costs: crate::permission::Costs::Free, cast, possible, from: CastOrigin::Fixed(Zone::Exile), rider: None, haste: false, one: true, ceiling: Some(bound) });
            e.instrs.push(Instr::Move { src: hit, to: Dest::Hand, by, when: Some(condition::Condition::outcome(cast, possible, ir::condition::Outcome::Didnt)) });
            e.instrs.push(Instr::Remainder { of: all, minus: vec![hit], still_in: Zone::Exile, out: rest });
            e.instrs.push(Instr::Move { src: rest, to: Dest::Library { position: LibraryPosition::Bottom, order: Some(Order::Random) }, by, when: None });
            e.instrs.push(Instr::Discovered { by, bound });
            // Keep the designation separate from the castable nonland hit (701.57c).
            let card = ObjectSet::of(Domain::Card);
            binder.result = Some((designated_subjects, card.clone(), 1));
            binder.name(designated_subjects, card, (0, 1), false);
            e.end_act(true, optional_branch);
            continue;
        }
        // Collecting is an object choice with a whole-group threshold, not an
        // ordinary exile or a cardinality (701.59a-b, 118.12).
        if let Action::Numbered(ir::NumberedAction::CollectEvidence, n) = &act.action {
            if binder.each.is_some() || !act.mods.is_empty() || !controller_only(clause.subject.as_ref()) {
                return unsupported("collect evidence: controller action without other modifiers required");
            }
            let players = e.clause_subject(clause, support, &mut binder)?;
            let (_, cards) = crate::cost::evidence_cost();
            let set = expr::Set::compile(&cards.set, support, &mut binder.scope(support))?;
            let action = choice::Action::collect_evidence(players.clone(), set, quantity(n, support, &binder)?);
            let (paid, possible) = (e.slot(), e.slot());
            binder.outcome = Some((players, paid, possible, e.instrs.len()));
            e.instrs.push(Instr::Choice { action, optional, payment: true, paid, possible, chosen: None, outcomes: None, actual: None, discarded_before: None });
            e.end_act(true, optional_branch);
            continue;
        }
        // "You may behold a Dragon. If you do, …": reveal a Dragon card from your hand or
        // choose a Dragon permanent you control (701.4a), a cost paid as the effect is applied,
        // answered by whether it was (118.12). Nothing is done to what is beheld, and no text
        // names it.
        if let Action::OnObjects(ir::action::ObjectAction::Behold, objects) = &act.action {
            if !optional || !alone || binder.each.is_some() || !act.mods.is_empty() || !controller_only(clause.subject.as_ref()) {
                return unsupported("behold: the controller's optional behold, alone in its clause, without other modifiers required");
            }
            let [selector] = objects.items.as_slice() else { return unsupported("behold: more than one kind") };
            if selector.quant != Quant::Indefinite(Count::Exactly(Amount::Const(1))) || selector.chooser.is_some() || selector.random.is_some() {
                return unsupported("behold: other than one");
            }
            let (cards, permanents) = crate::cost::beheld(selector)?;
            let players = e.clause_subject(clause, support, &mut binder)?;
            let cards = expr::Cards::compile(&cards, support, &mut binder.scope(support))?;
            let permanents = expr::Set::compile(&permanents, support, &mut binder.scope(support))?;
            let (paid, possible) = (e.slot(), e.slot());
            binder.outcome = Some((players.clone(), paid, possible, e.instrs.len()));
            e.instrs.push(Instr::Choice { action: choice::Action::behold(players, cards, permanents), optional, payment: true, paid, possible, chosen: None, outcomes: None, actual: None, discarded_before: None });
            e.end_act(false, optional_branch);
            continue;
        }
        if let Action::Numbered(ir::NumberedAction::Endure,n) = &act.action {
            if optional || binder.each.is_some() || act.mods.iter().any(|m| !matches!(m,ir::effect::Modifier::WhereX(_))) { return unsupported("endure: optional, for-each or other modifiers"); }
            let Some(Subject::Selector(subject)) = clause.subject.as_ref() else { return unsupported("endure: one explicitly named permanent"); };
            if !matches!(subject.quant,Quant::Definite(Ref::This|Ref::Anaphor(_))) || subject.chooser.is_some() || subject.random.is_some() { return unsupported("endure: one exact permanent reference"); }
            let permanent=e.operand(subject,support,&mut binder)?;
            let singular = match &permanent {
                action::Operand::Source => true,
                action::Operand::Reference(slot) => binder.subjects.iter().any(|named|
                    (named.slot == *slot || named.object == Some(*slot)) && named.bounds.1 <= 1),
                _ => false,
            };
            if !singular { return unsupported("endure: a singular permanent producer is required"); }
            e.instrs.push(Instr::Endure { permanent, amount: quantity(n,support,&binder)? });
            e.end_act(true,optional_branch);continue;
        }
        // 701.71a expands to conditional creation, a current battlefield choice,
        // then counter placement. Each step retains the ordinary operation owner;
        // creation replacements may produce several tokens or no eligible token.
        if let Action::Numbered(ir::NumberedAction::EmpowerJace, n) = &act.action {
            if optional || binder.each.is_some() || !controller_only(clause.subject.as_ref())
                || act.mods.iter().any(|m| !matches!(m, ir::effect::Modifier::WhereX(_))) {
                return unsupported("empower Jace: mandatory controller action without other modifiers required");
            }
            let player = e.clause_subject(clause, support, &mut binder)?;
            let you = Selector::new(Quant::Definite(Ref::You), ObjectSet::of(Domain::Player));
            let mut tokens = ObjectSet::of(Domain::Permanent);
            tokens.filter = ir::Filter::All(vec![
                ir::Filter::Atom(ir::Atom::Type(ir::CardType::Planeswalker)),
                ir::Filter::Atom(ir::Atom::Subtype(ir::Subtype::from_name("Jace").expect("Jace is a planeswalker subtype"))),
                ir::Filter::Atom(ir::Atom::Status(ir::Status::Token)),
            ]);
            tokens.controller = Some(ir::Relation { who: Box::new(you.clone()), negated: false, past: false });
            let condition = ir::Condition::Controls { player: Box::new(you),
                what: ir::Coord::one(Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))), tokens.clone())), negated: true };
            let condition = condition::Condition::compile(&condition, support, &mut binder.scope(support))?;
            let branch = e.instrs.len();
            e.instrs.push(Instr::Branch { condition, end: branch + 2, remember: None, skipped_values: Vec::new() });
            e.instrs.push(Instr::Action { action: action::Action::Create { players: player.clone(),
                tokens: vec![(action::Token::Defined(crate::token::empower_jace()?), expr::Amount::constant(1), false)],
                attacking: false, attached: None }, created: None, attempt: None });
            // The conditional creation is one written instruction boundary; this
            // checkpoint notes its events without SBA or priority during resolution.
            e.end_act(true, None);
            let set = expr::Set::compile(&tokens, support, &mut binder.scope(support))?;
            let (paid, possible, chosen) = (e.slot(), e.slot(), e.slot());
            e.instrs.push(Instr::Choice { action: choice::Action::choose(player.clone(), set, choice::Count::Exactly(expr::Amount::constant(1))),
                optional: false, payment: false, paid, possible, chosen: Some(chosen), outcomes: None, actual: None, discarded_before: None });
            // A current-state "where X" belongs to the final expanded instruction,
            // after creation/choice (608.2h). Historical amounts retain their bound
            // completed-action evidence instead (608.2i).
            if let Some(w) = defining.as_mut() { w.start = e.instrs.len(); }
            e.instrs.push(Instr::Action { action: action::Action::PutCounters { by: player,
                placements: vec![(action::Operand::Reference(chosen), vec![("loyalty".into(), quantity(n, support, &binder)?)])],
            }, created: None, attempt: None });
            e.end_act(true, optional_branch);
            continue;
        }
        // 701.46a: Adapt is the ordinary conditional counter operation.
        if let Action::Numbered(ir::NumberedAction::Adapt, n) = &act.action {
            if !act.mods.is_empty() || !controller_only(clause.subject.as_ref()) {
                return unsupported("adapt: explicit subject or modifiers");
            }
            let condition = ir::Condition::Is {
                what: Box::new(Selector::new(Quant::Definite(Ref::This), ObjectSet::of(Domain::Permanent))),
                pred: ir::condition::Predicate::Has(ir::Filter::Atom(ir::Atom::Counters {
                    kind: Some(ir::CounterKind("+1/+1".into())), count: ir::Count::Precisely(Amount::Const(0)),
                })), negated: false,
            };
            let condition = condition::Condition::compile(&condition, support, &mut binder.scope(support))?;
            let branch = e.instrs.len();
            e.instrs.push(Instr::Branch { condition, end: branch + 2, remember: None, skipped_values: Vec::new() });
            e.instrs.push(Instr::Action { action: action::Action::PutCounters { by: action::Operand::Controller,
                placements: vec![(action::Operand::Source, vec![("+1/+1".into(), quantity(n, support, &binder)?)])],
            }, created: None, attempt: None });
            e.end_act(false, optional_branch);
            continue;
        }
        if let Action::Scry(n) | Action::Surveil(n) = &act.action {
            let players = e.clause_subject(clause, support, &mut binder)?;
            let verb = if matches!(act.action, Action::Scry(_)) { Verb::Scry } else { Verb::Surveil };
            e.instrs.push(Instr::Arrange { players, verb, amount: quantity(n, support, &binder)?, optional });
            e.end_act(verb == Verb::Surveil, optional_branch);
            continue;
        }
        // "You may pay {B}. If you do, …" (118.12): the payment procedure, as a child
        if matches!(act.action, Action::PayMana(_) | Action::PayLife(_)) {
            if !optional { return unsupported("program: a payment an effect requires, not offers"); }
            if !single_actor(clause.subject.as_ref()) { return unsupported("optional payment by several players needs per-player outcomes"); }
            let payer = e.clause_subject(clause, support, &mut binder)?;
            let part = match &act.action { Action::PayMana(mana) => ir::CostPart::Mana(mana.clone()), _ => ir::CostPart::Action(act.action.clone()) };
            let cost = crate::cost::Plan::compile(&ir::Cost(vec![part]), support)?;
            if cost.x() > 0 && !support.x_available() { return unsupported("program: an effect's payment of {X} without a bound X"); }
            if !cost.phyrexian().is_empty() { return unsupported("program: an effect's payment of Phyrexian mana"); }
            let each = each.map(|n| quantity(n, support, &binder)).transpose()?;
            if each.is_some() { cost.clone().repeated_generic(1)?; }
            let paid = e.slot();
            // Only Did/Didnt are admitted for mana payments; both read `paid`.
            // Couldnt is refused above rather than fabricating a feasibility result.
            binder.outcome = Some((payer.clone(), paid, paid, e.instrs.len()));
            e.instrs.push(Instr::Pay { payer, cost, each, paid });
            e.end_act(false, optional_branch);
            continue;
        }
        // Explicit target declarations announce once (115.1a/d, 601.2c,
        // 602.2b, 603.3d); they are not choices made during resolution.
        // Let the common target compiler own counts, domains and relations,
        // and its Capture/Binder path retain each group's target provenance.
        if let Action::Choose(ir::action::Choice::Objects(objects)) = &act.action
            && objects.items.iter().any(|s| matches!(s.quant, Quant::Target(_)))
        {
            if optional || !controller_only(clause.subject.as_ref()) {
                return unsupported("target declaration: optional instruction or another chooser");
            }
            if objects.items.iter().any(|s| !matches!(s.quant, Quant::Target(_))) {
                return unsupported("target declaration: mixed target and resolution choices");
            }
            let before = binder.subjects.len();
            e.objects(objects, support, &mut binder)?;
            for named in &mut binder.subjects[before..] {
                if named.target { named.explicit_choice = true; }
            }
            e.end_act(false, optional_branch);
            continue;
        }
        // "Choose two cards in your hand drawn this turn": retain the private
        // selection before later per-card instructions, using the ordinary hand chooser.
        if let Action::Choose(ir::action::Choice::Objects(objects)) = &act.action
            && let [selector] = objects.items.as_slice()
            && matches!(selector.quant, Quant::Indefinite(Count::Exactly(Amount::Const(_)) | Count::UpTo(Amount::Const(_))))
            && selector.set.domain == Domain::Card && !selector.set.other
            && selector.set.zone.as_ref().is_some_and(|zone| zone.zones.items == [ir::Zone::Hand]
                && zone.slice.is_none() && zone.owner.as_deref().is_some_and(crate::select::is_you))
        {
            if optional || !controller_only(clause.subject.as_ref()) || selector.chooser.is_some()
                || selector.random.is_some() || !act.mods.is_empty() {
                return unsupported("choice in hand: one mandatory controller choice required");
            }
            e.chosen_from_hand(selector, Who::Controller, support, &mut binder)?;
            binder.subjects.last_mut().expect("private choice just named").explicit_choice = true;
            e.end_act(false, optional_branch);
            continue;
        }
        // "Choose an Army creature you control": of the permanents its player controls, as the
        // instruction is carried out (608.2d); "it" is then the one chosen
        if let Action::Choose(ir::action::Choice::Objects(objects)) = &act.action
            && let [selector] = objects.items.as_slice()
            && mention(selector).is_none()
            && let Quant::Indefinite(Count::Exactly(count)) = &selector.quant
            && selector.set.zone.is_none()
            && selector.set.controller.as_ref().is_some_and(|r| !r.negated && !r.past && crate::select::is_you(&r.who))
        {
            if optional || !controller_only(clause.subject.as_ref()) || selector.chooser.is_some() || selector.random.is_some() {
                return unsupported("choice of a permanent: optional, or by another player");
            }
            let players = e.clause_subject(clause, support, &mut binder)?;
            let set = expr::Set::compile(&selector.set, support, &mut binder.scope(support))?;
            let max = if let Amount::Const(n) = count { *n } else { u32::MAX };
            let count = choice::Count::Exactly(quantity(count, support, &binder)?);
            let (paid, possible, chosen, kept, subjects) = (e.slot(), e.slot(), e.slot(), e.slot(), e.slot());
            e.instrs.push(Instr::Choice { action: choice::Action::choose(players, set, count), optional: false, payment: false, paid, possible, chosen: Some(chosen), outcomes: None, actual: None, discarded_before: None });
            e.instrs.push(Instr::Gather { objects: action::Operand::Reference(chosen), out: kept });
            e.instrs.push(Instr::Subjects { cards: kept, out: subjects, still_in: None });
            binder.result = Some((subjects, selector.set.clone(), max));
            binder.name(subjects, selector.set.clone(), (0, max), false);
            binder.subjects.last_mut().expect("explicit choice just named").explicit_choice = true;
            e.end_act(false, optional_branch);
            continue;
        }
        if let Action::Exile(objects) = &act.action
            && let [selector] = objects.items.as_slice()
            && let Quant::Indefinite(Count::Exactly(count)) = &selector.quant
            && selector.set.domain == Domain::Card
            && selector.set.zone.as_ref().is_some_and(|z| z.owner_of(ir::Zone::Hand).is_some_and(crate::select::is_you))
        {
            if !controller_only(clause.subject.as_ref()) || selector.chooser.is_some() || selector.random.is_some() {
                return unsupported("selected exile: unsupported chooser");
            }
            let players = e.clause_subject(clause, support, &mut binder)?;
            let set = expr::Set::compile(&selector.set, support, &mut binder.scope(support))?;
            let count = choice::Count::Exactly(quantity(count, support, &binder)?);
            let instruction = choice::Action::exile_selected(players.clone(), set, count);
            let (paid, possible, chosen, moved, subjects) = (e.slot(), e.slot(), e.slot(), e.slot(), e.slot());
            binder.outcome = Some((players, paid, possible, e.instrs.len()));
            e.instrs.push(Instr::Choice { action: instruction, optional, payment: false, paid, possible, chosen: Some(chosen), outcomes: None, actual: None, discarded_before: None });
            e.instrs.push(Instr::Gather { objects: action::Operand::Reference(chosen), out: moved });
            e.instrs.push(Instr::Subjects { cards: moved, out: subjects, still_in: None });
            binder.exiled.push(subjects);
            binder.result = Some((subjects, selector.set.clone(), u32::MAX));
            e.end_act(true, optional_branch);
            continue;
        }
        if let (true, Action::Exile(objects)) = (named_exile, &act.action) {
            if !single_actor(clause.subject.as_ref()) { return unsupported("optional payment by several players needs per-player outcomes"); }
            let players = e.clause_subject(clause, support, &mut binder)?;
            let named = e.objects(objects, support, &mut binder)?;
            let (paid, possible) = (e.slot(), e.slot());
            binder.outcome = Some((players.clone(), paid, possible, e.instrs.len()));
            let (chosen, moved, subjects) = (e.slot(), e.slot(), e.slot());
            e.instrs.push(Instr::Choice { action: choice::Action::exile(players, named), optional, payment: false, paid, possible, chosen: Some(chosen), outcomes: None, actual: None, discarded_before: None });
            e.instrs.push(Instr::Gather { objects: action::Operand::Reference(chosen), out: moved });
            e.instrs.push(Instr::Subjects { cards: moved, out: subjects, still_in: None });
            binder.exiled.push(subjects);
            e.end_act(true, optional_branch);
            continue;
        }
        if act.action == Action::Keyword(ir::KeywordAction::Proliferate) {
            if !single_actor(clause.subject.as_ref()) { return unsupported("proliferate: one instructed player"); }
            let actor = e.clause_subject(clause, support, &mut binder)?;
            let by = match actor {
                action::Operand::Controller => Who::Controller,
                action::Operand::Reference(slot) => Who::Of(slot),
                actor => {
                    let slot = e.slot();
                    e.instrs.push(Instr::Capture { operand: actor, out: slot });
                    Who::Of(slot)
                }
            };
            e.instrs.push(Instr::Proliferate { by });
            e.end_act(false, optional_branch);
            continue;
        }
        if act.action==Action::Keyword(ir::KeywordAction::Connive) {
            if optional || each.is_some() || binder.each.is_some() {return unsupported("connive: optional, scaled or per-player forms");}
            let objects=e.clause_subject(clause,support,&mut binder)?;
            e.instrs.push(Instr::Connive{objects});e.end_act(true,optional_branch);continue;
        }
        if act.action==Action::Keyword(ir::KeywordAction::Explore) {
            if optional || each.is_some() || binder.each.is_some() {return unsupported("explore: optional, scaled or per-player forms");}
            let objects=e.clause_subject(clause,support,&mut binder)?;
            e.instrs.push(Instr::Explore{objects});e.end_act(true,optional_branch);continue;
        }
        if matches!(act.action, Action::Discard(_) | Action::Sacrifice(_)) {
            let players = e.clause_subject(clause, support, &mut binder)?;
            let actor = players.clone();
            let yours = actor == action::Operand::Controller;
            let (instruction, chosen) = e.object_choice(act, players, support, &mut binder)?;
            if optional { instruction.payment_ok()?; }
            if let Some((who, said)) = e.unless_said.take() {
                if optional { return unsupported("unless: an optional act"); }
                e.unless(&who, said, support, &mut binder)?;
            }
            let (paid, possible) = (e.slot(), e.slot());
            binder.outcome = Some((actor, paid, possible, e.instrs.len()));
            let outcomes = optional.then(|| [e.slot(), e.slot(), e.slot()]);
            if let Some(outputs) = outcomes { binder.player_outcomes.push(outputs); }
            let mut discarded_before = None;
            let actual = if matches!(act.action, Action::Discard(_)) {
                let out = e.slot();
                let before = e.slot(); discarded_before = Some(before); binder.discarded.push((out, before));
                binder.groups.push(Group { slot: out, paragraph: binder.paragraph(), by: Verb::Discard, zone: Zone::Graveyard, taken: Vec::new() });
                binder.produced.push(Produced { slot: out, verb: Verb::Discard, shape: expr::Shape::Cards, yours, into: Some(Zone::Graveyard) });
                Some(out)
            } else { None };
            e.instrs.push(Instr::Choice { action: instruction, optional, payment: false, paid, possible, chosen, outcomes, actual, discarded_before });
            e.end_act(true, optional_branch);
            continue;
        }
        if let Action::Search { zone, what } = &act.action {
            let Some((owner, zones)) = searched_zones(zone).filter(|_| controller_only(clause.subject.as_ref()) || binder.actor.is_some() || binder.each_does) else {
                return unsupported("search: only the controller searching their own library is implemented");
            };
            // the player the clause is said of searches "their" library (`done_by_its_player`)
            let by = binder.actor.map_or(Who::Controller, Who::Of);
            // "that player's library", of each player the text is said for: every one of them;
            // "target opponent's graveyard, hand, and library": the controller searches theirs,
            // looking at all of it (701.23a)
            // "Each player searches their library": each of them, their own (701.23i)
            let whose = if binder.each_does { binder.each.map(action::Operand::Reference) }
                else if binder.actor.is_some() || crate::select::is_you(owner) { None } else {
                if binder.each.is_none() && !single_actor(Some(&Subject::Selector(owner.clone()))) {
                    return unsupported("search: the zones of several players");
                }
                match e.operand(owner, support, &mut binder)? {
                    // "Search its controller's …. That player shuffles": the player whose
                    // zones are searched, named for what follows
                    whose @ (action::Operand::ControllerOf(_) | action::Operand::OwnerOf(_)) if binder.each.is_none() => {
                        let slot = e.slot();
                        e.instrs.push(Instr::Capture { operand: whose, out: slot });
                        binder.name(slot, ObjectSet::of(Domain::Player), (0, 1), false);
                        Some(action::Operand::Reference(slot))
                    }
                    whose => Some(whose),
                }
            };
            if binder.searched.is_some() { return unsupported("search: combined searches before one shuffle (701.23h)"); }
            let [selector] = what.items.as_slice() else { return unsupported("search: several separate descriptions"); };
            let action = search::Action::compile(selector, zones, support, &mut binder.scope(support))?;
            let out = e.slot();
            binder.result = Some((out, selector.set.clone(), action.max_bound()));
            binder.searched = Some(out);
            binder.searched_zone = action.source_zone();
            binder.found_waiting = true;
            e.instrs.push(Instr::Search { action, by, whose, own: binder.each_does, out });
            e.end_act(false, optional_branch);
            continue;
        }
        if let Action::Shuffle(ir::procedure::Shuffled::Default) = &act.action {
            let players = e.clause_subject(clause, support, &mut binder)?;
            let immediate_destination = following_destination(&acts[index + 1..]);
            if binder.found_waiting && !immediate_destination { return unsupported("search: shuffle before found cards have an immediate destination (701.24b)"); }
            let found = binder.searched.take().filter(|_| binder.found_waiting);
            e.instrs.push(Instr::Shuffle { players, found });
            e.end_act(false, optional_branch);
            continue;
        }
        // "its owner shuffles it into their library", "shuffle your graveyard into your
        // library": whoever's library is named, a card goes into its owner's (400.3)
        if let Action::ShuffleInto { what, whose: whose @ (Whose::Owner | Whose::Owners | Whose::Their | Whose::You | Whose::Player(_)), from: None } = &act.action {
            if optional { return unsupported("optional shuffle into a library"); }
            // Read the subject first: a nested target in "the owner of target
            // creature" supplies the following "it", and that owner is retained.
            e.clause_subject(clause, support, &mut binder)?;
            // Capture both the moved objects and their owners before moving anything (701.24).
            // Placement order is immaterial because each destination library is shuffled.
            let objects = e.objects(what, support, &mut binder)?;
            let cards = e.slot();
            e.instrs.push(Instr::Capture { operand: action::Operand::Union(objects), out: cards });
            // the library named is shuffled even when nothing is put into it (701.24d)
            let named = match whose {
                Whose::You => Some(action::Operand::Controller),
                Whose::Player(player) => Some(e.operand(player, support, &mut binder)?),
                Whose::Their => Some(their(&mut e, clause, support, &mut binder)?),
                _ => None,
            };
            let owners = e.slot();
            let players = std::iter::once(action::Operand::OwnerOf(Box::new(action::Operand::Reference(cards)))).chain(named).collect();
            e.instrs.push(Instr::Capture { operand: action::Operand::Union(players), out: owners });
            e.instrs.push(Instr::Place { objects: vec![action::Operand::Reference(cards)], position: ir::Position::One(ir::action::LibraryPosition::Top), chooser: None,
                order: None, attempt: None, by: None, before_shuffle: true });
            e.instrs.push(Instr::Shuffle { players: action::Operand::Reference(owners), found: None });
            e.end_act(true, optional_branch);
            continue;
        }
        if let Action::Until { verb: ir::action::UntilVerb::Exile, until } = &act.action {
            if optional || !act.mods.is_empty() || !controller_only(clause.subject.as_ref()) || binder.each.is_some()
                || until.quant != Quant::Indefinite(Count::Exactly(Amount::Const(1)))
                || until.chooser.is_some() || until.random.is_some()
                || until.set != (ObjectSet { filter: until.set.filter.clone(), ..ObjectSet::of(Domain::Card) }) {
                return unsupported("exile until: one nonland card below a mana-value bound in the controller's library required");
            }
            let ir::Filter::All(parts) = &until.set.filter else { return unsupported("exile until: a nonland mana-value bound required"); };
            let nonland = ir::Filter::Not(Box::new(ir::Filter::Atom(Atom::Type(ir::CardType::Land))));
            let bounds = parts.iter().filter_map(|part| match part {
                ir::Filter::Atom(Atom::Stat { stat: ir::Stat::ManaValue, cmp: ir::amount::Cmp::Lt, value }) => Some((value, ScanLimit::LessThan)),
                ir::Filter::Atom(Atom::Stat { stat: ir::Stat::ManaValue, cmp: ir::amount::Cmp::Le, value }) => Some((value, ScanLimit::AtMost)),
                _ => None,
            }).collect::<Vec<_>>();
            let [(value, limit)] = bounds.as_slice() else { return unsupported("exile until: one mana-value bound required"); };
            if parts.len() != 2 || !parts.contains(&nonland) { return unsupported("exile until: additional stopping constraints not implemented"); }
            let (value, limit) = ((*value).clone(), *limit);
            let amount = quantity(&value, support, &binder)?;
            let (all, hit, designated, bound, subjects) = (e.slot(), e.slot(), e.slot(), e.slot(), e.slot());
            e.instrs.push(Instr::LibraryScan { by: Who::Controller, amount, limit, all, hit, designated, bound });
            e.instrs.push(Instr::Subjects { cards: hit, out: subjects, still_in: Some(Zone::Exile) });
            binder.groups.push(Group { slot: all, paragraph: binder.paragraph(), by: Verb::Exile, zone: Zone::Exile, taken: Vec::new() });
            binder.result = Some((subjects, until.set.clone(), 1));
            binder.name(subjects, until.set.clone(), (0, 1), false);
            e.end_act(true, optional_branch);
            continue;
        }
        if let Action::Until { verb: ir::action::UntilVerb::Reveal, until } = &act.action {
            if optional || !act.mods.is_empty() || !controller_only(clause.subject.as_ref())
                || until.quant != Quant::Indefinite(Count::Exactly(Amount::Const(1)))
                || until.chooser.is_some() || until.random.is_some() || until.set.other {
                return unsupported("reveal until: one matching card in the controller's library required");
            }
            let set = binder.selection(&until.set, support)?;
            let (all, matched, shown, subjects) = (e.slot(), e.slot(), e.slot(), e.slot());
            e.instrs.push(Instr::TakeUntil { set, whose: action::Operand::Controller, out: all, matched });
            e.instrs.push(Instr::Show { from: all, by: Who::Controller, to: Audience::Everyone, out: shown });
            e.instrs.push(Instr::Subjects { cards: matched, out: subjects, still_in: None });
            binder.groups.push(Group { slot: shown, paragraph: binder.paragraph(), by: Verb::Reveal, zone: Zone::Library, taken: vec![matched] });
            binder.result = Some((subjects, until.set.clone(), 1));
            binder.name(subjects, until.set.clone(), (0, 1), false);
            e.end_act(false, optional_branch);
            continue;
        }
        if let Action::Reveal(objects) | Action::LookAt(objects) = &act.action
            && objects.items.iter().all(|s| top_of_library(s).is_none()) {
            e.clause_subject(clause, support, &mut binder)?;
            let operands = if let [selector] = objects.items.as_slice()
                && let Some(mention) = mention(selector)
            {
                if optional { return unsupported("optional selected reveal requires a retained optional scope"); }
                let set = binder.selection(&mention.set, support)?;
                e.unite(&mention.reference, &mut binder);
                let group = binder.resolve(&mention.reference)?;
                let (from, zone) = (group.slot, group.zone);
                let pick = match mention.taking {
                    Taking::Exactly(n) => Pick::Exactly(n),
                    Taking::UpTo(n) => Pick::UpTo(n),
                    Taking::Quantity(_) => return unsupported("variable selected reveal not lowered"),
                    Taking::AnyNumber => Pick::AnyNumber,
                    Taking::All => Pick::All,
                    Taking::Remainder => return unsupported("revealing a remainder is not a selection"),
                };
                let chooser = binder.actor.map_or(Who::Controller, Who::Of);
                let (operand, selected) = e.select_cards(from, set, mention.set, pick, zone, chooser, &mut binder);
                binder.resolve(&mention.reference)?.taken.push(selected);
                vec![operand]
            } else { e.objects(objects, support, &mut binder)? };
            let zone = match operands.as_slice() {
                [action::Operand::Zone(zone)] => Some(zone.zone()),
                // A search result is retained in its one source zone until a later
                // movement. Revealing those exact subjects supplies a public group.
                [action::Operand::Reference(slot)] if binder.searched == Some(*slot) && binder.found_waiting => binder.searched_zone,
                _ => None,
            };
            let out = e.slot();
            let looking = matches!(act.action, Action::LookAt(_));
            if looking {
                let from = e.slot();
                let by = binder.actor.map_or(Who::Controller, Who::Of);
                e.instrs.push(Instr::Gather { objects: action::Operand::Union(operands), out: from });
                e.instrs.push(Instr::Show { from, by, to: Audience::Only(by), out });
            } else {
                e.instrs.push(Instr::Observe { objects: action::Operand::Union(operands), by: binder.actor.map_or(Who::Controller, Who::Of), out });
            }
            let by = if looking { Verb::LookAt } else { Verb::Reveal };
            if let Some(zone) = zone { binder.groups.push(Group { slot: out, paragraph: binder.paragraph(), by, zone, taken: Vec::new() }); }
            // An observation of a previously bound object is a producer just as an
            // observation of the library top is. Declining an optional reveal
            // initializes its skipped result to an empty group.
            binder.produced.push(Produced { slot: out, verb: by, shape: expr::Shape::Cards,
                yours: controller_only(clause.subject.as_ref()), into: None });
            e.end_act(false, optional_branch);
            continue;
        }
        // "Manifest dread" (701.62a): the player looks at the top two cards of their library,
        // manifests one of them, then puts the cards they looked at that were not manifested
        // into their graveyard, the one they chose too if it could not enter (701.40f).
        // "Manifest dread twice": the whole of it, then again. "Manifest the top two cards of
        // your library", "cloak two of them" (of cards the player looked at): one card at a
        // time (701.40e, 701.58e), each chosen as it is put.
        let dread = act.action == Action::Keyword(ir::KeywordAction::ManifestDread);
        let (top_cards, of_group) = match &act.action {
            Action::OnObjects(verb @ (ir::action::ObjectAction::Manifest | ir::action::ObjectAction::Cloak), objects) => match objects.items.as_slice() {
                [selector] => match (top_of_library(selector), mention(selector)) {
                    (Some((Amount::Const(n), owner)), _) if *n > 1 => (Some((*n, owner, concealment(*verb))), None),
                    (_, Some(mention)) => (None, Some((mention, concealment(*verb)))),
                    _ => (None, None),
                },
                _ => (None, None),
            },
            _ => (None, None),
        };
        if dread || top_cards.is_some() || of_group.is_some() {
            let by = e.face_down_actor(clause, support, &mut binder)?;
            let times = act.mods.iter().find_map(|m| match m { ir::effect::Modifier::Times(Amount::Const(n)) => Some(*n), _ => None }).unwrap_or(1);
            let mut permanents = Vec::new();
            if let Some((mention, concealment)) = of_group {
                let Taking::Exactly(n) = mention.taking else { return unsupported("manifest or cloak: a fixed number of the cards named"); };
                let set = binder.selection(&mention.set, support)?;
                e.unite(&mention.reference, &mut binder);
                let group = binder.resolve(&mention.reference)?;
                let (from, zone) = (group.slot, group.zone);
                // each choice is a card not chosen before, whether or not that one could enter
                // (608.2d); one that couldn't stays where it was (701.40f, 701.58f), so "the
                // rest" still includes it: those put leave the zone, and leave the rest so
                let mut prior = Vec::new();
                for _ in 0..n {
                    let (among, chosen, card) = (e.slot(), e.slot(), e.slot());
                    e.instrs.push(Instr::Remainder { of: from, minus: prior.clone(), still_in: zone, out: among });
                    e.instrs.push(Instr::Select { from: among, set: set.clone().into(), pick: Pick::Exactly(1), chooser: by, gate: None, still_in: zone, out: chosen });
                    prior.push(chosen);
                    e.instrs.push(Instr::Subjects { cards: chosen, out: card, still_in: None });
                    binder.name(card, ObjectSet::of(Domain::Card), (0, 1), false);
                    permanents.push(e.enter_face_down(card, by, concealment, &mut binder));
                }
            } else if let Some((n, owner, concealment)) = top_cards {
                let whose = e.operand(owner, support, &mut binder)?;
                for _ in 0..n {
                    let (taken, card) = (e.slot(), e.slot());
                    e.instrs.push(Instr::TakeTop { n: expr::Amount::constant(1), whose: whose.clone(), out: taken });
                    e.instrs.push(Instr::Subjects { cards: taken, out: card, still_in: None });
                    permanents.push(e.enter_face_down(card, by, concealment, &mut binder));
                }
            } else {
                let whose = match by { Who::Controller => action::Operand::Controller, Who::Of(slot) => action::Operand::Reference(slot) };
                let any = expr::Cards::compile(&ObjectSet::of(Domain::Card), support, &mut binder.scope(support))?;
                for _ in 0..times {
                    let (taken, looked, chosen, card, rest) = (e.slot(), e.slot(), e.slot(), e.slot(), e.slot());
                    e.instrs.push(Instr::TakeTop { n: expr::Amount::constant(2), whose: whose.clone(), out: taken });
                    e.instrs.push(Instr::Show { from: taken, by, to: Audience::Only(by), out: looked });
                    e.instrs.push(Instr::Select { from: looked, set: any.clone().into(), pick: Pick::Exactly(1), chooser: by, gate: None, still_in: Zone::Library, out: chosen });
                    e.instrs.push(Instr::Subjects { cards: chosen, out: card, still_in: None });
                    binder.name(card, ObjectSet::of(Domain::Card), (0, 1), false);
                    permanents.push(e.enter_face_down(card, by, crate::face::Concealment::Manifested, &mut binder));
                    e.instrs.push(Instr::Remainder { of: looked, minus: Vec::new(), still_in: Zone::Library, out: rest });
                    e.instrs.push(Instr::Move { src: rest, to: Dest::Graveyard, by, when: None });
                }
            }
            // what the text goes on to call "that creature" or "those creatures"
            let max = permanents.len() as u32;
            let out = match permanents.as_slice() {
                [one] => *one,
                _ => {
                    let out = e.slot();
                    e.instrs.push(Instr::Capture { operand: action::Operand::Union(permanents.into_iter().map(action::Operand::Reference).collect()), out });
                    out
                }
            };
            binder.result = Some((out, ObjectSet::of(Domain::Permanent), max));
            binder.name(out, ObjectSet::of(Domain::Permanent), (0, max), false);
            e.end_act(true, optional_branch);
            continue;
        }
        // "search … for a card and exile it, then shuffle": what was found has left the
        // library before the shuffle
        if let Action::Exile(found) = &act.action
            && binder.searched.is_some()
            && matches!(found.items.as_slice(), [s] if matches!(s.quant, Quant::Definite(Ref::Anaphor(_))))
        {
            binder.found_waiting = false;
        }
        if let Action::Move { moves: groups, from: None, .. } = &act.action {
            binder.found_waiting = false;
            if !result_move && groups.len() == 1 {
                let (objects, to) = &groups[0];
                let library = match to {
                    // "on the bottom of that player's library": a card goes only to its owner's (400.3)
                    Destination::Library(ir::LibraryPlace { library: Some(Whose::You | Whose::Owner | Whose::Owners | Whose::Player(_)), back: false, position, order }) => Some((position.clone(), *order, None)),
                    // "target opponent puts a card from their hand on top of their library":
                    // the player the clause is said of, into their own library (400.3)
                    Destination::Library(ir::LibraryPlace { library: Some(Whose::Their), back: false, position, order })
                        if single_actor(clause.subject.as_ref()) => Some((position.clone(), *order, None)),
                    Destination::Library(ir::LibraryPlace { library: None, back: false, position, order }) if binder.result.is_some() => Some((position.clone(), *order, None)),
                    // "Look at the top three cards of [target player's] library, then put them
                    // back in any order": the cards the look took, into the library they are
                    // still in, in the order the player told to put them chooses
                    Destination::Library(ir::LibraryPlace { library: None, back: true, position, order })
                        if binder.result.is_some() && binder.groups.last().is_some_and(|g| g.zone == Zone::Library) =>
                    {
                        if !controller_only(clause.subject.as_ref()) { return unsupported("putting back looked-at cards by another player"); }
                        Some((position.clone(), *order, Some(Who::Controller)))
                    }
                    _ => None,
                };
                if let Some((position, order, mut by)) = library {
                    let chooser = match &position {
                        ir::Position::BeneathTopX => return unsupported("library destination: beneath the top X cards"),
                        ir::Position::One(_) => None,
                        ir::Position::Choice { chooser, .. } => match chooser {
                            Some(Whose::Owner | Whose::Owners) => return unsupported("library destination choice requires one explicit actor, not owner-group partitions"),
                            Some(Whose::You) => Some(action::Operand::Controller),
                            None | Some(Whose::Their) => Some(e.clause_subject(clause, support, &mut binder)?),
                            Some(Whose::Player(player)) => Some(e.operand(player, support, &mut binder)?),
                        },
                    };
                    let mut actor = action::Operand::Controller;
                    // "up to one target player puts all the cards from their graveyard …": the
                    // player the clause is said of is named before what they put, so "their"
                    // is theirs and not the unsaid controller's (608.2c)
                    if !controller_only(clause.subject.as_ref()) && single_actor(clause.subject.as_ref()) {
                        e.clause_subject(clause, support, &mut binder)?;
                    }
                    let objects = if let [selector] = objects.items.as_slice()
                        && let Quant::Indefinite(Count::Exactly(Amount::Const(n))) = selector.quant
                        && selector.set.domain == Domain::Card && selector.set.zone.is_some()
                    {
                        // 608.2d/401.4: choose from the hand as this instruction begins,
                        // then let the ordinary placement procedure collect the order.
                        let zone = selector.set.zone.as_ref().unwrap();
                        // the controller from their hand, or the one player the clause is
                        // said of from theirs
                        let other = !controller_only(clause.subject.as_ref());
                        if optional || selector.chooser.is_some() || selector.random.is_some()
                            || zone.zones.items.as_slice() != [ir::Zone::Hand]
                            || if other { !single_actor(clause.subject.as_ref()) || binder.each.is_some() || !zone.owner.as_deref().is_some_and(|o| o.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::They))) }
                               else { !zone.owner.as_deref().is_some_and(crate::select::is_you) }
                        { return unsupported("selected library placement: the controller's hand, or the hand of the one player told to"); }
                        let chooser = if other {
                            match e.clause_subject(clause, support, &mut binder)? {
                                action::Operand::Reference(slot) => Who::Of(slot),
                                operand => {
                                    let slot = e.slot();
                                    e.instrs.push(Instr::Capture { operand, out: slot });
                                    binder.name(slot, ObjectSet::of(Domain::Player), (0, 1), false);
                                    binder.clause = Some(expr::Subject::Local(slot));
                                    Who::Of(slot)
                                }
                            }
                        } else { Who::Controller };
                        if let Who::Of(slot) = chooser { actor = action::Operand::Reference(slot); by = by.or(Some(chooser)); }
                        let cards = expr::ZoneCards::compile(&selector.set, support, &mut binder.scope(support))?;
                        let from = e.slot();
                        e.instrs.push(Instr::Gather { objects: action::Operand::Zone(cards), out: from });
                        let description = ObjectSet { zone: None, ..selector.set.clone() };
                        let set = binder.selection(&description, support)?;
                        vec![e.select_cards(from, set, description, Pick::Exactly(n as usize), Zone::Hand, chooser, &mut binder).0]
                    } else { e.objects(objects, support, &mut binder)? };
                    // "on top of target player's library": that player is announced and bound
                    // though a card goes to its owner's (400.3, 601.2c)
                    if let Destination::Library(ir::LibraryPlace { library: Some(Whose::Player(player)), .. }) = to { e.operand(player, support, &mut binder)?; }
                    if let Destination::Library(ir::LibraryPlace { library: Some(Whose::Their), .. }) = to { their(&mut e, clause, support, &mut binder)?; }
                    // "put them on top of that player's library in any order": the player told
                    // to puts them in the order they choose, not the library's owner (101.1, 401.4)
                    if by.is_none() && order == Some(ir::action::Order::Any)
                        && matches!(to, Destination::Library(ir::LibraryPlace { library: Some(Whose::Player(_) | Whose::Their), .. }))
                    {
                        by = Some(match &actor { action::Operand::Reference(slot) => Who::Of(*slot), _ => Who::Controller });
                    }
                    if optional && !controller_only(clause.subject.as_ref()) { return unsupported("optional library placement by another actor"); }
                    let (paid, possible) = (e.slot(), e.slot());
                    binder.outcome = Some((actor.clone(), paid, possible, e.instrs.len()));
                    let attempt = Some(Attempt { actor, optional, payment: false, distributed: true, paid, possible, count: None, cost: false });
                    e.instrs.push(Instr::Place { objects, position, chooser, order, attempt, by, before_shuffle: false });
                    e.end_act(true, optional_branch);
                    continue;
                }
            }
        }
        let mut optional_selection = false;
        let instruction = match &act.action {
            Action::Move { verb: said, moves: groups, from: None } if !result_move => {
                moves = true;
                if groups.len() > 1 && groups.iter().any(|(objects, _)| objects.items.iter().any(|s| matches!(s.quant, Quant::Indefinite(_)) && chosen_on_resolution(&s.set).is_some())) {
                    return unsupported("selected object: one description");
                }
                let groups = groups.iter().map(|(objects, to)| {
                    let zone = match to {
                        Destination::Hand(Whose::Owner | Whose::Owners | Whose::You) => action::Destination::Zone(Zone::Hand),
                        Destination::Graveyard(Whose::Owner | Whose::Owners | Whose::You) => action::Destination::Zone(Zone::Graveyard),
                        Destination::Hand(Whose::Their | Whose::Player(_)) | Destination::Graveyard(Whose::Their | Whose::Player(_)) => {
                            // "into their graveyard", "into that player's hand": a card goes
                            // to its owner's hand or graveyard whichever is named (400.3)
                            action::Destination::Zone(if matches!(to, Destination::Hand(_)) { Zone::Hand } else { Zone::Graveyard })
                        },
                        Destination::Battlefield(entries) => {
                            // another player puts it there only as the player a clause is said of,
                            // or as each of the players it is said of, what they found
                            if !controller_only(clause.subject.as_ref()) && binder.actor.is_none() && !binder.each_does { return unsupported("battlefield movement by another player not lowered"); }
                            battlefield_destination(entries, support, &binder)?
                        },
                        _ => return unsupported("program: movement destination not lowered"),
                    };
                    let operands = if optional && alone && groups.len() == 1
                        && controller_only(clause.subject.as_ref()) && binder.actor.is_none() && binder.each.is_none()
                        && act.mods.is_empty() && e.unless_said.is_none()
                        && matches!(zone, action::Destination::Battlefield { .. })
                        && matches!(objects.items.as_slice(), [s] if s.quant == Quant::Indefinite(Count::Exactly(Amount::Const(1)))
                            && s.set.domain == Domain::Card && s.chooser.is_none() && s.random.is_none()
                            && s.set.zone.as_ref().is_some_and(|z| z.owner_of(ir::Zone::Hand).is_some_and(crate::select::is_you)))
                    {
                        // Consent and the card are one choice. No effect or new information
                        // lies between them; declining does not choose a private card (402.3).
                        let s = &objects.items[0];
                        let set = expr::Set::compile(&s.set, support, &mut binder.scope(support))?;
                        let (from, selected, subjects, paid, possible) = (e.slot(), e.slot(), e.slot(), e.slot(), e.slot());
                        // Keep the hand pool until Select: its existence is public, its
                        // qualifying identities are not (402.3).
                        e.instrs.push(Instr::Gather { objects: action::Operand::All(set.unfiltered()), out: from });
                        binder.outcome = Some((action::Operand::Controller, paid, possible, e.instrs.len()));
                        e.instrs.push(Instr::Select { from, set: SelectionFilter::Action { set, verb: Verb::Move, role: ir::Role::What, enters: true },
                            pick: Pick::Optional { count: 1, paid, possible }, chooser: Who::Controller, gate: None, still_in: Zone::Hand, out: selected });
                        e.instrs.push(Instr::Subjects { cards: selected, out: subjects, still_in: None });
                        binder.name(subjects, s.set.clone(), (0, 1), false);
                        optional_selection = true;
                        vec![action::Operand::Reference(subjects)]
                    } else if matches!(zone, action::Destination::Zone(Zone::Hand))
                        && objects.items.iter().any(|s| matches!(s.quant, Quant::Indefinite(_)) && s.set.domain == Domain::Permanent)
                    {
                        // "you may return any number of …": choosing none is not returning
                        let may_none = objects.items.iter().all(|s| matches!(s.quant, Quant::Indefinite(Count::AnyNumber | Count::UpTo(_))));
                        if (optional && !may_none) || binder.each.is_some() || !single_actor(clause.subject.as_ref()) || objects.items.len() != 1 || groups.len() != 1 {
                            return unsupported("selected return: one mandatory actor and object group, no per-player iteration");
                        }
                        let actor = e.clause_subject(clause, support, &mut binder)?;
                        let chooser = match actor {
                            action::Operand::Controller => Who::Controller,
                            action::Operand::Reference(slot) => Who::Of(slot),
                            actor => {
                                let slot = e.slot();
                                e.instrs.push(Instr::Capture { operand: actor, out: slot });
                                binder.name(slot, ObjectSet::of(Domain::Player), (0,1), false);
                                binder.clause = Some(expr::Subject::Local(slot));
                                Who::Of(slot)
                            },
                        };
                        objects.items.iter().map(|s| {
                            if matches!(s.quant, Quant::Indefinite(_)) {
                                e.select_permanents(s, chooser, Verb::Move, support, &mut binder)
                            } else { e.object(s, support, &mut binder) }
                        }).collect::<Res<_>>()?
                    } else { e.chosen_objects(objects, clause, optional, Verb::Move, ir::Role::What, !matches!(zone, action::Destination::Zone(_)), support, &mut binder)? };
                    // "to target player's hand": that player is announced and bound though the
                    // card goes to its owner's (400.3, 601.2c)
                    if let Destination::Hand(Whose::Player(player)) | Destination::Graveyard(Whose::Player(player)) = to { e.operand(player, support, &mut binder)?; }
                    if let Destination::Hand(Whose::Their) | Destination::Graveyard(Whose::Their) = to { their(&mut e, clause, support, &mut binder)?; }
                    Ok((operands, zone))
                }).collect::<Res<_>>()?;
                Some(action::Action::Move { groups, said: *said })
            }
            Action::Exile(objects) if until_source => {
                moves = true;
                Some(action::Action::ExileUntil { objects: e.objects(objects, support, &mut binder)? })
            }
            Action::Prevent(p) => {
                let mods = duration_mods(clause, act, &durations);
                let turn = p.this_turn.is_some() || mods.iter().any(|m| matches!(m,
                    ir::effect::Modifier::Duration(ir::effect::Duration::ThisTurn)
                    | ir::effect::Modifier::Duration(ir::effect::Duration::Until(ir::event::Time { end: true, step: ir::event::Step::Turn, whose: ir::event::StepOwner::This }))));
                if !turn || mods.iter().any(|m| !matches!(m, ir::effect::Modifier::Duration(ir::effect::Duration::ThisTurn)
                    | ir::effect::Modifier::Duration(ir::effect::Duration::Until(ir::event::Time { end: true, step: ir::event::Step::Turn, whose: ir::event::StepOwner::This })))) {
                    return unsupported("prevention: only this-turn duration");
                }
                let next = match &p.amount { ir::Prevented::Next(Amount::Const(n)) => Some(*n), ir::Prevented::All => None,
                    _ => return unsupported("prevention: fixed next-N or all") };
                if next.is_some() && (p.by.is_some() || !matches!(&p.to, Some(party) if matches!(party.items.as_slice(),
                    [ir::statics::Who::One(s)] if matches!(s.quant, Quant::Target(Count::Exactly(Amount::Const(1))))))) {
                    return unsupported("prevention shield: one target, no source restriction");
                }
                let mut party = |party: &Option<ir::statics::Party>| -> Res<Vec<action::Operand>> {
                    let Some(party) = party else { return Ok(Vec::new()) };
                    if party.conj != ir::Conj::And && party.items.len()>1 { return unsupported("prevention: alternative parties"); }
                    party.items.iter().map(|who| match who {
                        ir::statics::Who::One(s) => e.operand(s,support,&mut binder),
                        ir::statics::Who::Set(set) => Ok(action::Operand::All(expr::Set::compile_root(set,support,&mut binder.scope(support))?)),
                    }).collect()
                };
                if p.by.as_ref().is_some_and(|p|p.items.iter().any(|who|match who {
                    ir::statics::Who::One(s)=>kinds(&s.set).0,
                    ir::statics::Who::Set(set)=>kinds(set).0,
                })) {return unsupported("prevention: damage sources cannot be players");}
                if p.to==p.by && p.to.as_ref().is_some_and(|p|p.items.iter().any(|w|matches!(w,
                    ir::statics::Who::One(Selector{quant:Quant::Target(_),..})))) {
                    return unsupported("prevention: ambiguous target announcement in coupled to/by parties");
                }
                let to=party(&p.to)?; let by=party(&p.by)?;
                Some(action::Action::Prevent {to,by,to_or_by:p.to_or_by,kind:p.kind,next})
            }
            Action::Regenerate(objects) => Some(action::Action::Regenerate { objects: e.objects(objects, support, &mut binder)? }),
            Action::Attach { what, to } => {
                if matches!(&to.quant, Quant::Target(Count::Exactly(Amount::Const(1)) | Count::UpTo(Amount::Const(1))) | Quant::Definite(_)) {
                    // "Return this card from your graveyard to the battlefield, then attach it
                    // to that creature": the pronoun is the object the card became, which this
                    // effect can find (400.7j). The source the binder keeps is the card as it
                    // was, so the attachment would do nothing: refused until the moved object
                    // is followed.
                    let moved_source = std::iter::once(&clause.first).chain(clause.rest.iter().map(|(_, act)| act)).any(|act| {
                        matches!(&act.action, Action::Move { moves, .. }
                            if moves.iter().any(|(objects, _)| objects.items.iter().any(|s| s.quant == Quant::Definite(Ref::This))))
                    });
                    if moved_source && what.items.iter().any(|s| s.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::It))) {
                        return unsupported("attach: a pronoun for the card this effect moved (400.7j)");
                    }
                    let objects = e.objects(what, support, &mut binder)?;
                    Some(action::Action::Attach { objects, to: e.operand(to, support, &mut binder)? })
                } else { return unsupported("attach: choosing between multiple destinations"); }
            }
            // (of a group the text took, "exile the rest": below, as chosen cards)
            Action::Exile(objects) if !objects.items.iter().any(|s| mention(s).is_some()) && objects.items.iter().all(|s| s.quant != Quant::The
                || (s.set.domain == Domain::Card && s.set.zone.as_ref().is_some_and(|zone| zone.slice.is_none()))) => {
                moves = true;
                Some(action::Action::Move { groups: vec![(e.chosen_objects(objects, clause, optional, Verb::Exile, ir::Role::What, false, support, &mut binder)?, action::Destination::Zone(Zone::Exile))], said: ir::action::MoveVerb::Put })
            }
            Action::Destroy(objects) => {
                moves = true;
                let regenerable = !matches!(acts.get(index + 1), Some(TextStep::Act(c, a, _)) if bars_regeneration(c, a) && !a.may);
                skip_rider = !regenerable;
                Some(action::Action::Destroy { objects: e.chosen_objects(objects, clause, optional, Verb::Destroy, ir::Role::What, false, support, &mut binder)?, regenerable })
            }
            Action::Counter(objects) => {
                moves = true;
                let objects=e.objects(objects,support,&mut binder)?;
                // The replacement rider and this counter instruction have one owner;
                // do not move to the graveyard and then try to find the spell again.
                let mut destination=action::CounterDestination::Graveyard;
                if let Some([TextStep::If(condition, true),TextStep::Act(clause,rider,true),TextStep::EndIf])=acts.get(index+1..index+4)
                    && !matches!(acts.get(index+4), Some(TextStep::Otherwise | TextStep::Instead(_)))
                    && let Some((named, to)) = counter_destination_rider(condition,clause,rider)
                {
                    let subject=e.operand(countered_subject(condition).expect("rider proved"),support,&mut binder)?;
                    let named=e.objects(named,support,&mut binder)?;
                    if objects==[subject] && named==objects {destination=to;reach.skip_through=index+4;}
                }
                Some(action::Action::Counter { objects, destination })
            }
            Action::Keyword(ir::action::KeywordAction::BecomeMonarch) => Some(action::Action::BecomeMonarch { players: e.clause_subject(clause, support, &mut binder)? }),
            // said of no subject: the speed of the player whose ability it is (702.179d)
            Action::Keyword(ir::action::KeywordAction::IncreaseSpeed) if clause.subject.is_none() => Some(action::Action::IncreaseSpeed { players: action::Operand::Controller }),
            Action::Draw(n) | Action::DrawAdditional(n) | Action::DrawUpTo(n) => Some(action::Action::Draw { players: e.clause_subject(clause, support, &mut binder)?, amount: player_quantity(n, support, &binder)? }),
            Action::Mill(n) => Some(action::Action::Mill { players: e.clause_subject(clause, support, &mut binder)?, amount: player_quantity(n, support, &binder)? }),
            Action::GainLife(n) => Some(action::Action::GainLife { players: e.clause_subject(clause, support, &mut binder)?, amount: player_quantity(n, support, &binder)? }),
            Action::Have { who, action } if matches!(**action,Action::LoseLife(_)) => {
                let Action::LoseLife(n) = &**action else { unreachable!() };
                Some(action::Action::LoseLife { players: e.operand(who,support,&mut binder)?, amount: player_quantity(n,support,&binder)? })
            }
            Action::LoseLife(n) => Some(action::Action::LoseLife { players: e.clause_subject(clause, support, &mut binder)?, amount: player_quantity(n, support, &binder)? }),
            Action::Tap(objects) | Action::Untap(objects) => {
                let operands = if let [selector] = objects.items.as_slice()
                    && matches!(selector.quant, Quant::Indefinite(_)) {
                    if !controller_only(clause.subject.as_ref()) || binder.actor.is_some() || optional && matches!(selector.quant, Quant::Indefinite(Count::Exactly(_))) {
                        return unsupported("bounded tap selection: controller chooses a mandatory action");
                    }
                    vec![e.select_permanents(selector, Who::Controller,
                        if matches!(act.action, Action::Tap(_)) { Verb::Tap } else { Verb::Untap }, support, &mut binder)?]
                } else { e.objects(objects, support, &mut binder)? };
                Some(action::Action::Tap { objects: operands, tapped: matches!(act.action, Action::Tap(_)) })
            },
            Action::DealDamage(hits) => {
                if hits.iter().any(|hit| hit.condition.is_some()) {
                    return unsupported("damage: independently conditional components are not lowered (608.2c)");
                }
                // "If this spell was bargained, instead it deals 3 damage to that permanent":
                // what deals the damage is not what it is dealt to, which would be "itself"
                // (the converse of a recipient "it"); the "it" is the source
                let it = |s: &Selector| s.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::It));
                let dealer_is_source = matches!(clause.subject.as_ref(), Some(Subject::Selector(s)) if it(s))
                    && binder.subject(&Selector::new(Quant::Definite(Ref::Anaphor(ir::Anaphor::It)), ObjectSet::of(Domain::Object))).is_ok_and(|dealer| !dealer.is_empty()
                        && hits.iter().all(|h| h.to.items.iter().all(|to| matches!(to.quant, Quant::Definite(Ref::Anaphor(ir::Anaphor::That | ir::Anaphor::The { .. })))
                            && binder.subject(to).is_ok_and(|named| named == dealer))));
                let source = if clause.subject.is_none() || dealer_is_source { binder.clause = Some(expr::Subject::Source); action::Operand::Source }
                    else {
                        match e.clause_subject_scoped(clause, support, &mut binder, true)? {
                            // "This creature gets +1/+0 until end of turn and deals 1 damage to you."
                            action::Operand::Reference(slot) if e.captured_source == Some(slot) => action::Operand::Source,
                            subject => subject,
                        }
                    };
                // CR608.2h/120.1: a distributive union supplies one object actor per
                // member, while retaining its separately announced target groups.
                // Mixed player/object unions have no such quantity evaluation contract.
                fn object_dealers(subject: &Subject) -> bool {
                    match subject {
                        Subject::Selector(s) | Subject::Each(s) => kinds(&s.set) == (false, true),
                        Subject::Set(s) => kinds(s) == (false, true),
                        Subject::And { items, .. } => !items.is_empty() && items.iter().all(object_dealers),
                        _ => false,
                    }
                }
                if matches!(clause.subject.as_ref(), Some(Subject::And { each: true, .. }))
                    && !clause.subject.as_ref().is_some_and(object_dealers) {
                    return unsupported("distributive damage: combined dealers must all be objects");
                }
                let object_each = match clause.subject.as_ref() {
                    Some(s @ Subject::And { each: true, .. }) => object_dealers(s),
                    Some(Subject::Each(s)) => kinds(&s.set) == (false, true),
                    Some(Subject::Set(s)) => kinds(s) == (false, true),
                    Some(Subject::Selector(s)) if matches!(s.quant, Quant::Each | Quant::All) => kinds(&s.set) == (false, true),
                    _ => false,
                };
                let hits = hits.iter().map(|h| {
                    // 608.2h: the named object does the damage. “Each other creature”
                    // excludes that actor, not the spell/ability whose effect this is.
                    // Preserve source-relative defaults for self-naming/implicit sources,
                    // and restore the binding before quantities or subsequent instructions.
                    let context = RecipientContext { subject: match &source {
                        action::Operand::Reference(slot) => Some(expr::Subject::Local(*slot)),
                        action::Operand::Subject(subject) => Some(subject.clone()),
                        _ => None,
                    }, distributive: object_each };
                    let (to, recipient_players) = binder.with_recipients(context, |binder| e.damage_recipients(&h.to, support, binder))?;
                    let each_player = h.to.items.iter().all(|s| s.set.domain == Domain::Player && matches!(s.quant, Quant::Each | Quant::All));
                    let mut amounts = binder.clone();
                    // The IR already distinguishes distributive subjects. Only their own
                    // possessors become a member; explicit references and group totals stay bound.
                    amounts.object_each = object_each;
                    let amount = if each_player {
                        amounts.clause = Some(expr::Subject::Actor);
                        // each player dealt damage is the actor; a per-source quantity is not
                        expr::Amount::compile(&h.amount, support, &mut Scoped { binder: &amounts, support, actor: true, object_actor: false, to_players: true })?
                    } else {
                        expr::Amount::compile(&h.amount, support, &mut Scoped {
                            binder: &amounts, support, actor: false,
                            // Player-recipient expressions keep their existing actor path.
                            // Do not admit a source-member quantity that it cannot evaluate.
                            object_actor: !recipient_players, to_players: recipient_players,
                        })?
                    };
                    Ok((to, amount))
                }).collect::<Res<_>>()?;
                Some(action::Action::Damage { source, hits })
            }
            // "deals 3 damage divided as you choose among one, two, or three targets" (601.2d)
            Action::DealDivided { amount, among, division: ir::action::DamageDivision::Chosen } => {
                let source = if clause.subject.is_none() { binder.clause = Some(expr::Subject::Source); action::Operand::Source }
                    else {
                        match e.clause_subject_scoped(clause, support, &mut binder, true)? {
                            action::Operand::Reference(slot) if e.captured_source == Some(slot) => action::Operand::Source,
                            subject => subject,
                        }
                    };
                let (among, _) = e.damage_recipients(among, support, &mut binder)?;
                let group = e.divided_group(&among, amount, support, &binder)?;
                Some(action::Action::DealDivided { source, group })
            }
            // "distribute three +1/+1 counters among one, two, or three target creatures" (601.2d)
            Action::Distribute { counters, among } => {
                if counters.up_to || counters.extra.is_some() { return unsupported("distribute: optional or additional counters"); }
                let among = e.chosen_objects(among, clause, optional, Verb::Distribute, ir::Role::Among, false, support, &mut binder)?;
                let group = e.divided_group(&among, &counters.count, support, &binder)?;
                Some(action::Action::Distribute { by: e.clause_subject(clause, support, &mut binder)?, group, kind: counters.kind.0.clone() })
            }
            Action::Fight(objects) => {
                let one = if clause.subject.is_none() { binder.clause = Some(expr::Subject::Source); action::Operand::Source }
                    else { e.clause_subject(clause, support, &mut binder)? };
                let other = e.objects(objects, support, &mut binder)?;
                for operand in std::iter::once(&one).chain(other.iter()) {
                    if let action::Operand::Reference(slot) = operand { binder.fight_recipients.push(*slot); }
                }
                Some(action::Action::Fight { one, other })
            }
            // a class level bar's ability (716.2a)
            Action::Becomes(ir::action::Becoming::Level(n)) => {
                let object = if clause.subject.is_none() { binder.clause = Some(expr::Subject::Source); action::Operand::Source }
                    else { e.clause_subject(clause, support, &mut binder)? };
                Some(action::Action::Level { objects: vec![object], level: *n })
            }
            // "it becomes monstrous" (701.37a), "it becomes renowned" (702.112a), "this
            // permanent becomes saddled until end of turn" (702.171a)
            Action::Becomes(b @ (ir::action::Becoming::Solved | ir::action::Becoming::Monstrous | ir::action::Becoming::Renowned | ir::action::Becoming::Saddled | ir::action::Becoming::Prepared)) => {
                let object = if clause.subject.is_none() { binder.clause = Some(expr::Subject::Source); action::Operand::Source }
                    else { e.clause_subject(clause, support, &mut binder)? };
                use ir::condition::ObjectDesignation as D;
                let designation = match b { ir::action::Becoming::Solved => D::Solved, ir::action::Becoming::Monstrous => D::Monstrous, ir::action::Becoming::Renowned => D::Renowned, ir::action::Becoming::Prepared => D::Prepared, _ => D::Saddled };
                Some(action::Action::Designate { objects: vec![object], designation })
            }
            Action::Becomes(ir::action::Becoming::Unprepared) => {
                let object = if clause.subject.is_none() { binder.clause = Some(expr::Subject::Source); action::Operand::Source }
                    else { e.clause_subject(clause, support, &mut binder)? };
                Some(action::Action::Unprepare { objects: vec![object] })
            }
            Action::FightEachOther => Some(action::Action::Fight { one: e.clause_subject(clause, support, &mut binder)?, other: Vec::new() }),
            // converting is transforming by another name (701.28a)
            Action::OnObjects(ir::action::ObjectAction::Transform | ir::action::ObjectAction::Convert, objects) => Some(action::Action::Transform { objects: e.objects(objects, support, &mut binder)? }),
            // "Turn target face-down creature face up" (708.8): no cost is paid, and a
            // manifested instant or sorcery is revealed instead (701.40g). "Turn this creature
            // face down": a 2/2 with no text, as no characteristics are listed (708.2a).
            Action::TurnFace { what, up } => Some(action::Action::TurnFace { objects: e.objects(what, support, &mut binder)?, up: *up }),
            // Manifest or cloak the top card of a library, or a card from the player's hand
            // (701.40a, 701.58a): that card is put onto the battlefield face down, under the
            // control of the player who manifests it, who chooses a card from their hand;
            // "it" is then the face-down permanent (400.7j).
            Action::OnObjects(verb @ (ir::action::ObjectAction::Manifest | ir::action::ObjectAction::Cloak), objects) => {
                let [selector] = objects.items.as_slice() else { return unsupported("manifest or cloak: several groups of cards"); };
                let by = e.face_down_actor(clause, support, &mut binder)?;
                let card = if let Some((Amount::Const(1), owner)) = top_of_library(selector) {
                    let whose = e.operand(owner, support, &mut binder)?;
                    let (taken, card) = (e.slot(), e.slot());
                    e.instrs.push(Instr::TakeTop { n: expr::Amount::constant(1), whose, out: taken });
                    e.instrs.push(Instr::Subjects { cards: taken, out: card, still_in: None });
                    binder.name(card, ObjectSet::of(Domain::Card), (0, 1), false);
                    action::Operand::Reference(card)
                } else if selector.quant == Quant::Indefinite(Count::Exactly(Amount::Const(1)))
                    && selector.set.domain == Domain::Card && selector.chooser.is_none() && selector.random.is_none()
                    && selector.set.zone.as_ref().is_some_and(|zone| zone.slice.is_none() && zone.zones.items.as_slice() == [ir::Zone::Hand]
                        && zone.owner.as_deref().is_some_and(crate::select::is_you) && by == Who::Controller)
                {
                    // the card is chosen from the hand as the instruction begins (608.2d); the
                    // other players see a face-down 2/2 arrive and no card
                    let cards = expr::ZoneCards::compile(&selector.set, support, &mut binder.scope(support))?;
                    let from = e.slot();
                    e.instrs.push(Instr::Gather { objects: action::Operand::Zone(cards), out: from });
                    let description = ObjectSet { zone: None, ..selector.set.clone() };
                    let set = binder.selection(&description, support)?;
                    e.select_cards(from, set, description, Pick::Exactly(1), Zone::Hand, by, &mut binder).0
                } else {
                    return unsupported("manifest or cloak: only the top card of a library or a card from a hand");
                };
                moves = true;
                Some(action::Action::Move { groups: vec![(vec![card], action::Destination::Battlefield {
                    owner_controls: false, by, tapped: false, counters: Vec::new(), attacking: None, transformed: false,
                    face_down: Some(concealment(*verb)),
                })], said: ir::action::MoveVerb::Put })
            }
            // "… deals 4 damage instead": to what the act it replaces would have dealt its to
            Action::Static(s) if replaces && matches!(**s, ir::StaticAct::DealsToSame(_)) => {
                let ir::StaticAct::DealsToSame(n) = &**s else { unreachable!() };
                let dealt = e.instrs.iter().rev().find_map(|i| match i { Instr::Action { action, .. } => Some(action), _ => None });
                let Some(action::Action::Damage { source, hits }) = dealt else { return unsupported("damage dealt instead of something other than damage"); };
                let [(to, _)] = hits.as_slice() else { return unsupported("damage dealt instead of several amounts of damage"); };
                Some(action::Action::Damage { source: source.clone(), hits: vec![(to.clone(), quantity(n, support, &binder)?)] })
            }
            Action::GetCounters { count, kind } => {
                let on = e.clause_subject(clause, support, &mut binder)?;
                if on == action::Operand::Source { return unsupported("player counters: an object where a player gets counters"); }
                let kind = expr::player_counter_kind(kind)?;
                Some(action::Action::PutCounters { by: action::Operand::Controller, placements: vec![(on, vec![(kind, player_quantity(count, support, &binder)?)])] })
            }
            Action::RemoveCounters { count, kind: Some(kind), from: Some(from), among: false } => {
                let count = match count { ir::CounterCount::Exactly(Amount::Const(n)) => Some(*n), ir::CounterCount::All => None,
                    _ => return unsupported("counter removal: fixed count or all required") };
                let holder = e.operand(from, support, &mut binder)?;
                fn bound_player(holder: &action::Operand, binder: &Binder) -> bool {
                    match holder {
                        action::Operand::Controller | action::Operand::Player(_) | action::Operand::ControllerOf(_) | action::Operand::OwnerOf(_) => true,
                        action::Operand::Reference(slot) => binder.subjects.iter().find(|n| n.slot == *slot).is_none_or(|n| kinds(&n.set).0),
                        action::Operand::Union(items) => items.iter().any(|o| bound_player(o, binder)),
                        _ => false,
                    }
                }
                let player_holder = kinds(&from.set).0 || bound_player(&holder, &binder);
                if player_holder { expr::player_counter_kind(&ir::PlayerCounter::Named(kind.clone()))?; }
                else { crate::layers::counter_ok(&kind.0)?; }
                Some(action::Action::RemoveCounters { from: holder, counters: action::CounterRemoval::Named { kind: kind.0.clone(), count } })
            }
            Action::RemoveCounters { count: ir::CounterCount::All, kind: None, from: Some(from), among: false } => {
                let holder = e.operand(from, support, &mut binder)?;
                Some(action::Action::RemoveCounters { from: holder, counters: action::CounterRemoval::All })
            }
            Action::RemoveCounters { .. } => return unsupported("counter removal: named fixed/all count or all kinds, explicit holder and no among required"),
            // to pay {E}, a player removes an energy counter from themselves (107.14)
            Action::PayEnergy(Amount::Const(n)) => {
                let payer = e.clause_subject(clause, support, &mut binder)?;
                Some(action::Action::RemoveCounters { from: payer, counters: action::CounterRemoval::Named { kind: "energy".into(), count: Some(*n) } })
            }
            Action::PayEnergy(_) => return unsupported("energy payment: a fixed amount required"),
            Action::PutCounters(placements) if placements.len() > 1 && placements.iter().any(|p| matches!(p.on.quant, Quant::Indefinite(_)) && chosen_on_resolution(&p.on.set).is_some()) =>
                return unsupported("selected object: one description"),
            Action::PutCounters(placements) => Some(action::Action::PutCounters { by: e.clause_subject(clause, support, &mut binder)?, placements: placements.iter().map(|p| {
                let on = e.chosen_objects(&ir::Objects { items: vec![(*p.on).clone()], conj: ir::Conj::And }, clause, optional, Verb::PutCounters, ir::Role::On, false, support, &mut binder)?
                    .pop().expect("one holder");
                let players_in = |set: &ObjectSet| set.possibilities().iter().any(|p| p.domain.sort == ir::Sort::Player);
                let player_holder = players_in(&p.on.set) || match &on {
                    action::Operand::Controller | action::Operand::Player(_) | action::Operand::ControllerOf(_) | action::Operand::OwnerOf(_) => true,
                    action::Operand::Reference(slot) => binder.subjects.iter().find(|n| n.slot == *slot).is_none_or(|n| players_in(&n.set)),
                    _ => false,
                };
                let counters = p.counters.iter().map(|c| {
                    if c.up_to || c.extra.is_some() { return unsupported("program: optional/additional counter count not lowered"); }
                    if player_holder { expr::player_counter_kind(&ir::PlayerCounter::Named(c.kind.clone()))?; }
                    else { crate::layers::counter_ok(&c.kind.0)?; }
                    Ok((c.kind.0.clone(), quantity(&c.count, support, &binder)?))
                }).collect::<Res<_>>()?;
                Ok((on, counters))
            }).collect::<Res<_>>()? }),
            // "Populate": create a token that's a copy of a creature token you control (701.36a)
            Action::Keyword(ir::KeywordAction::Populate) => {
                let you = Selector::new(Quant::Definite(Ref::You), ObjectSet::of(Domain::Player));
                let mut tokens = ObjectSet::of(Domain::Permanent);
                tokens.filter = ir::Filter::All(vec![ir::Filter::Atom(ir::Atom::Type(ir::CardType::Creature)), ir::Filter::Atom(ir::Atom::Status(ir::Status::Token))]);
                tokens.controller = Some(ir::Relation { who: Box::new(you), negated: false, past: false });
                let of = Selector::new(Quant::Indefinite(Count::Exactly(Amount::Const(1))), tokens);
                let from = e.chosen_copy_source(&of, clause, optional, support, &mut binder)?;
                moves = true;
                Some(action::Action::Create { players: e.clause_subject(clause, support, &mut binder)?,
                    tokens: vec![(action::Token::Copy { of: Box::new(from), changes: Vec::new() }, expr::Amount::constant(1), false)], attacking: false, attached: None })
            }
            Action::CreateCopy { count, tapped, attacking: false, of }
                if matches!(of.quant, Quant::Definite(Ref::This | Ref::Anaphor(ir::Anaphor::That | ir::Anaphor::It)) | Quant::Target(Count::Exactly(Amount::Const(1)) | Count::UpTo(Amount::Const(1))))
                    || (of.quant == Quant::Indefinite(Count::Exactly(Amount::Const(1)))
                        && (expr::linked_exile_filter(&of.set.filter).is_some() || chosen_on_resolution(&of.set) == Some(Zone::Battlefield))) => {
                let from = if of.quant == Quant::Indefinite(Count::Exactly(Amount::Const(1))) && expr::linked_exile_filter(&of.set.filter).is_none() {
                    e.chosen_copy_source(of, clause, optional, support, &mut binder)?
                } else if expr::linked_exile_filter(&of.set.filter).is_some() && matches!(of.quant, Quant::Indefinite(Count::Exactly(Amount::Const(1)))) {
                    let set = expr::Set::compile(&of.set, support, &mut binder.scope(support))?;
                    let (from, selected, subjects) = (e.slot(), e.slot(), e.slot());
                    e.instrs.push(Instr::Gather { objects: action::Operand::All(set), out: from });
                    e.instrs.push(Instr::Select { from, set: expr::Cards::compile(&ObjectSet::of(Domain::Card), support, &mut binder.scope(support))?.into(), pick: Pick::Exactly(1), chooser: Who::Controller, gate: None, still_in: Zone::Exile, out: selected });
                    e.instrs.push(Instr::Subjects { cards: selected, out: subjects, still_in: None });
                    action::Operand::Reference(subjects)
                } else { e.operand(of, support, &mut binder)? };
                if matches!(&from,action::Operand::Reference(slot) if binder.created_groups.contains(slot)) {
                    return unsupported("token copy: a multiplied created group is not a singular copy source");
                }
                let changes = act.mods.iter().filter_map(|m| if let ir::Modifier::Except(e) = m { Some(e) } else { None })
                    .map(|exceptions| crate::copy::changes(exceptions, true)).collect::<Res<Vec<_>>>()?.into_iter().flatten().collect();
                moves = true;
                Some(action::Action::Create { players: e.clause_subject(clause, support, &mut binder)?,
                    tokens: vec![(action::Token::Copy { of: Box::new(from), changes }, quantity(count, support, &binder)?, *tapped)], attacking: false, attached: None })
            }
            Action::GetEmblem(granted) => Some(action::Action::GetEmblem {
                players: e.clause_subject(clause, support, &mut binder)?, definition: crate::ability::emblem(granted)?,
            }),
            Action::Create(tokens) => {
                if tokens.items.len() > 1 && tokens.conj == ir::Conj::Or {
                    if optional || !act.mods.is_empty() || each.is_some() || !controller_only(clause.subject.as_ref()) {
                        return unsupported("token alternatives require a mandatory controller instruction without modifiers");
                    }
                    let mut actions = Vec::new();
                    let mut options = Vec::new();
                    for (index, token) in tokens.items.iter().enumerate() {
                        let ir::token::TokenKind::Predefined(kind) = token.kind else { return unsupported("token alternatives require predefined tokens"); };
                        let Amount::Const(count) = token.count else { return unsupported("token alternatives require fixed quantities"); };
                        if token.tapped || token.combat.is_some() || token.attached_to.is_some() { return unsupported("token alternative entry modifiers are not implemented"); }
                        let values = crate::token::token_values(&token.kind)?;
                        actions.push(action::Action::Create { players: action::Operand::Controller,
                            tokens: vec![(action::Token::Defined(values), expr::Amount::constant(count), false)], attacking: false, attached: None });
                        options.push(crate::state::ActionOption { index, body: crate::state::ActionOptionBody::Create { tokens: vec![(kind, count)] } });
                    }
                    binder.result = None;
                    binder.alternative_result = true;
                    e.instrs.push(Instr::Alternative { actions, options });
                    e.end_act(true, None);
                    continue;
                }

                let result_controller = matches!(clause.subject.as_ref(), Some(Subject::Selector(s))
                    if s.set == ObjectSet::of(Domain::Player) && s.chooser.is_none() && s.random.is_none()
                    && matches!(&s.quant, Quant::Definite(Ref::ControllerOf(of))
                        if of.quant == Quant::Definite(Ref::Anaphor(ir::Anaphor::It)) && kinds(&of.set) == (false, true)))
                    && matches!(each, Some(Amount::Count(_)));
                let (players, multiplier) = if result_controller {
                    let Some(Amount::Count(set)) = each.take() else { unreachable!() };
                    let (slot, filter) = expr::Amount::result_controllers(set, support, &mut binder.scope(support))?;
                    let count = expr::Amount::death_count_by_actor(slot, filter.clone());
                    (action::Operand::FormerControllers(action::FormerControllers { slot, filter }), Some(count))
                } else { (e.clause_subject(clause, support, &mut binder)?, None) };
                if result_controller {
                    e.clause_subject = Some((players.clone(), Ok(())));
                    binder.clause = Some(expr::Subject::Actor);
                }
                // "create two 1/1 Soldier tokens that are tapped and attacking": the controller
                // chooses what each attacks as it enters (508.4)
                let attacking = tokens.items.iter().any(|t| matches!(t.combat, Some(ir::token::Combat::Attacking(None))));
                if attacking && (tokens.items.len() != 1 || players != action::Operand::Controller || each.is_some() || multiplier.is_some()) {
                    return unsupported("tokens entering attacking: one kind, created by the controller, no per-member quantity");
                }
                // "Create a Monster Role token attached to it": an Aura put onto the battlefield
                // attached to what the effect names (303.4f, 303.4i)
                let attached = match tokens.items.as_slice() {
                    [t @ ir::token::TokenSpec { attached_to: Some(to), .. }] => {
                        if players != action::Operand::Controller || each.is_some() || multiplier.is_some() || !matches!(t.count, Amount::Const(1)) {
                            return unsupported("token attached: one, created by the controller");
                        }
                        if !matches!(&to.quant, Quant::Target(Count::Exactly(Amount::Const(1)) | Count::UpTo(Amount::Const(1))) | Quant::Definite(_)) {
                            return unsupported("token attached: to other than one object");
                        }
                        // a predefined token the engine does not create says so (111.10)
                        let aura = match &t.kind {
                            ir::token::TokenKind::Predefined(kind) => crate::attach::aura(crate::token::definition(*kind)?),
                            _ => false,
                        };
                        if !aura { return unsupported("token attached: other than an Aura"); }
                        Some(e.operand(to, support, &mut binder)?)
                    }
                    _ => None,
                };
                let tokens: Vec<_> = tokens.items.iter().map(|t| {
                    if !matches!(t.combat, None | Some(ir::token::Combat::Attacking(None))) || (t.attached_to.is_some() && attached.is_none()) { return unsupported("program: token combat/attachment not lowered"); }
                    if multiplier.is_some() && !matches!(t.count, Amount::Const(_)) { return unsupported("result creation: per-member token quantity not implemented"); }
                    let amount = quantity(&t.count, support, &binder)?;
                    let amount = multiplier.clone().map_or(amount.clone(), |each| amount.times(each));
                    Ok((action::Token::Defined(crate::token::token_values(&t.kind)?), amount, t.tapped))
                }).collect::<Res<_>>()?;
                moves = true;
                Some(action::Action::Create { players, tokens, attacking, attached })
            }
            // "Investigate" means "Create a Clue token" (701.16a): the predefined token of 111.10f
            Action::Keyword(ir::KeywordAction::Investigate) => {
                let clue = crate::token::token_values(&ir::token::TokenKind::Predefined(ir::Predefined::Clue))?;
                moves = true;
                Some(action::Action::Create { players: e.clause_subject(clause, support, &mut binder)?,
                    tokens: vec![(action::Token::Defined(clue), expr::Amount::constant(1), false)], attacking: false, attached: None })
            }
            _ => None,
        };
        if let Some(instruction) = instruction {
            let instruction = if let Some(each) = each {
                let amount = if matches!(act.action, Action::Draw(_) | Action::DrawAdditional(_) | Action::DrawUpTo(_) | Action::Mill(_) | Action::GainLife(_) | Action::LoseLife(_) | Action::GetCounters { .. }) {
                    player_quantity(each, support, &binder)?
                } else { quantity(each, support, &binder)? };
                instruction.scaled(amount)?
            } else { instruction };
            instruction.check()?;
            // "investigate twice": the keyword action done that many times, each an action of
            // its own (701.16a); none "zero times". Times is accepted for Investigate alone.
            let times = act.mods.iter().find_map(|m| match m { ir::effect::Modifier::Times(Amount::Const(n)) => Some(*n), _ => None }).unwrap_or(1);
            let investigate = matches!(act.action, Action::Keyword(ir::KeywordAction::Investigate));
            let created = match instruction.output() {
                Some(action::Output::Countered) => {
                    let out = e.slot();
                    let action::Action::Counter { objects, .. } = &instruction else { unreachable!("counter result producer") };
                    if let [subject] = objects.as_slice() { binder.countered.push((subject.clone(), out)); }
                    Some(out)
                },
                Some(action::Output::Damage) => { let out = e.slot(); binder.damage.push(out); Some(out) },
                Some(action::Output::RemovedCounters) => { let out = e.slot(); binder.removed.push(out); Some(out) },
                Some(action::Output::Destroyed) => {
                    let out = e.slot(); binder.destroyed.push(out); binder.produced.push(Produced { slot: out, verb: Verb::Destroy, shape: expr::Shape::Destroyed, yours: controller_only(clause.subject.as_ref()), into: Some(Zone::Graveyard) }); Some(out)
                },
                Some(action::Output::Created { max }) => {
                    let out = e.slot();
                    binder.created_groups.push(out);
                    binder.created_single = (max <= 1 && times <= 1).then_some(out);
                    binder.result = Some((out, ObjectSet::of(ir::Domain::Permanent), u32::MAX));
                    Some(out)
                }
                Some(action::Output::LostLife) => {
                    let out = e.slot(); binder.lost.push(out); Some(out)
                }
                Some(action::Output::Moved { destination: Some(Zone::Battlefield) }) => {
                    let out = e.slot(); binder.entered.push(out); binder.produced.push(Produced { slot: out, verb: Verb::Move, shape: expr::Shape::Moved, yours: controller_only(clause.subject.as_ref()), into: Some(Zone::Battlefield) }); Some(out)
                }
                Some(action::Output::Moved { destination: Some(Zone::Exile) }) => {
                    let out = e.slot(); binder.exiled.push(out); binder.produced.push(Produced { slot: out, verb: Verb::Exile, shape: expr::Shape::Moved, yours: controller_only(clause.subject.as_ref()), into: Some(Zone::Exile) }); Some(out)
                }
                // "return", "put": a move into another zone (400.7)
                Some(action::Output::Moved { destination }) => {
                    let out = e.slot(); binder.produced.push(Produced { slot: out, verb: Verb::Move, shape: expr::Shape::Moved, yours: controller_only(clause.subject.as_ref()), into: destination }); Some(out)
                }
                Some(action::Output::Milled { controller }) => {
                    let out = e.slot();
                    if controller { binder.groups.push(Group { slot: out, paragraph: binder.paragraph(), by: Verb::Mill, zone: Zone::Graveyard, taken: Vec::new() }); }
                    binder.produced.push(Produced { slot: out, verb: Verb::Mill, shape: expr::Shape::Cards, yours: controller, into: Some(Zone::Graveyard) });
                    Some(out)
                }
                _ => None,
            };
            let attempt = if ordinary_attempt && !optional_selection {
                if optional && !controller_only(clause.subject.as_ref()) && !matches!(act.action, Action::GetCounters { .. }) && !matches!(&instruction, action::Action::Draw {..} | action::Action::Mill {..} | action::Action::GainLife {..} | action::Action::LoseLife {..} | action::Action::Create {..} | action::Action::GetEmblem {..}) {
                    return unsupported("optional object action by several actors not lowered");
                }
                let actor = e.clause_subject(clause, support, &mut binder)?;
                let (paid, possible) = (e.slot(), e.slot());
                binder.outcome = Some((actor.clone(), paid, possible, e.instrs.len()));
                Some(Attempt { actor, optional, payment: false, distributed: !causative_loss, paid, possible,
                    count: draw_bound.map(|max| BoundedCount { max, out: e.slot() }), cost: matches!(act.action, Action::PayEnergy(_)) })
            } else { None };
            // the payment is offered after the act's targets are bound, before it is done
            if let Some((who, said)) = e.unless_said.take() { e.unless(&who, said, support, &mut binder)?; }
            // Retain only a proven named input, and only in texts that read its past.
            // Gather follows this occurrence's own public transitions; Subjects keeps
            // that incarnation after the movement. Its departure snapshot already owns
            // the characteristics (608.2h/i), so no second LKI store is needed.
            if past_reads && let action::Action::Move { groups, .. } = &instruction {
                for (objects, _) in groups {
                    for object in objects {
                        let action::Operand::Reference(old) = object else { continue };
                        let names: Vec<_> = binder.subjects.iter()
                            .filter(|named| named.slot == *old || named.object == Some(*old))
                            .map(|named| named.slot).chain(std::iter::once(*old)).collect();
                        let (cards, input) = (e.slot(), e.slot());
                        e.instrs.push(Instr::Gather { objects: object.clone(), out: cards });
                        e.instrs.push(Instr::Subjects { cards, out: input, still_in: None });
                        binder.past_inputs.extend(names.into_iter().map(|name| (name, input)));
                    }
                }
            }
            if investigate && times != 1 {
                // each time is an action of its own, creating a Clue; what they created is all
                // of those together, and nothing when it was done zero times
                let made: Vec<Slot> = (0..times).map(|_| {
                    let one = e.slot();
                    e.instrs.push(Instr::Action { action: instruction.clone(), created: Some(one), attempt: attempt.clone() });
                    one
                }).collect();
                let out = created.expect("investigating has a result");
                e.instrs.push(Instr::Capture { operand: action::Operand::Union(made.into_iter().map(action::Operand::Reference).collect()), out });
            } else {
                e.instrs.push(Instr::Action { action: instruction, created, attempt });
            }
            // 400.7j: a later anaphor referring to the one object this move named
            // acts on its completed entry result. Preserve the original named card for
            // characteristic amounts, including failed entry (608.2h; Reanimate ruling).
            if let Some(cards) = created && binder.entered.last() == Some(&cards)
                && let Some(Instr::Action { action: action::Action::Move { groups, .. }, .. }) = e.instrs.last()
                && let [(objects, _)] = groups.as_slice()
                && let [action::Operand::Reference(old)] = objects.as_slice()
            {
                let old = *old;
                let subjects = e.slot();
                e.instrs.push(Instr::Subjects { cards, out: subjects, still_in: Some(Zone::Battlefield) });
                for named in &mut binder.subjects {
                    if named.slot == old || named.object == Some(old) { named.object = Some(subjects); }
                }
                // the card a search found, which no subject names (701.23a)
                if binder.result.as_ref().is_some_and(|(result, ..)| *result == old) && !binder.subjects.iter().any(|n| n.slot == old) {
                    binder.result_entered = Some((old, subjects));
                }
            }
            if let Some(cards) = created && binder.exiled.last() == Some(&cards) {
                let (subjects, now) = (e.slot(), e.slot());
                e.instrs.push(Instr::Subjects { cards, out: subjects, still_in: None });
                e.instrs.push(Instr::Subjects { cards, out: now, still_in: Some(Zone::Exile) });
                *binder.exiled.last_mut().unwrap() = subjects;
                binder.exiled_now.push(now);
            }
            e.end_act(moves, optional_branch);
            continue;
        }
        // "add an additional {G}": mana added as any is, by a triggered mana ability (106.12a)
        if let Action::AddMana(spec) | Action::AddAdditionalMana(spec) = &act.action {
            let players = e.clause_subject(clause, support, &mut binder)?;
            let mut production = if stated.is_some() || x_defined
                || matches!(spec, ir::mana::ManaSpec::Of { color: ir::mana::ManaColor::Chosen, .. })
                || binder.condition_amount.is_some()
                && matches!(spec, ir::mana::ManaSpec::AmountOf { amount: Amount::That { many: false }, .. }) {
                mana::Production::compile_bound(spec, support, &mut binder.scope(support))?
            } else { mana::Production::compile(spec, support)? };
            if let Some(n) = each { production = production.each(quantity(n, support, &binder)?)?; }
            if let Some(rider) = crate::mana::spend_only(effect) { production = production.spend_only(rider, support)?; }
            if non_generic { production = production.non_generic()?; }
            if let Some(kept) = crate::mana::kept(effect) { production = production.kept(kept); }
            e.instrs.push(Instr::Mana { players, production });
            e.end_act(false, optional_branch);
            continue;
        }
        // "An opponent separates those cards into two piles.": the cards of the group the
        // text made, where they are (700.3c). An explicit whole-pile chooser overrides the controller.
        if let Action::SeparateIntoPiles { what, piles: ir::procedure::Piles::Count(Amount::Const(2)) } = &act.action
            && let [selector] = what.items.as_slice()
        {
            if optional || !durations.is_empty() || !act.mods.is_empty() || binder.piles.is_some() { return Err(not_lowered(&act.action)); }
            let opponent = match clause.subject.as_ref() {
                Some(Subject::Selector(s)) if crate::select::is_an_opponent(s) && s.chooser.is_none() && s.random.is_none() => true,
                subject if controller_only(subject) => false,
                _ => return unsupported("piles: separated by other than the controller or an opponent"),
            };
            let (from, still_in, retained) = if matches!(selector.quant, Quant::Definite(Ref::Anaphor(ir::Anaphor::Those | ir::Anaphor::Them)))
                && selector.set.filter == ir::Filter::All(Vec::new()) {
                let group = binder.resolve(&UnresolvedRef::cards(RefForm::Anaphor))?;
                (group.slot, group.zone, true)
            } else if matches!(selector.quant, Quant::All | Quant::Each) && selector.set.domain == Domain::Permanent {
                let objects = e.operand(selector, support, &mut binder)?;
                let from = e.slot();
                e.instrs.push(Instr::Gather { objects, out: from });
                (from, Zone::Battlefield, false)
            } else { return unsupported("piles: a retained card group or a whole permanent group required"); };
            let chooser = match acts.get(index + 1) {
                Some(TextStep::Act(_, next, _)) if piles::chooser(&next.action).is_some() => {
                    let chooser = piles::chooser(&next.action).expect("a stated chooser");
                    if chooser.set.domain != Domain::Player || !matches!(chooser.quant, Quant::Definite(_)) {
                        return unsupported("piles: one named player's choice required");
                    }
                    e.operand(chooser, support, &mut binder)?
                },
                _ => action::Operand::Controller,
            };
            let (one, other) = (e.slot(), e.slot());
            if retained { binder.resolve(&UnresolvedRef::cards(RefForm::Anaphor))?.taken.extend([one, other]); }
            e.instrs.push(Instr::Piles { from, still_in, opponent, chooser, one, other });
            binder.piles = Some((one, other, still_in));
            e.end_act(false, optional_branch);
            continue;
        }
        // "Put one pile into your hand and the other on the bottom of your library in any
        // order."
        if let Action::MovePile(parts) = &act.action
            && let [(ir::procedure::PileRef::One, first), (ir::procedure::PileRef::TheOther, second)] = parts.as_slice()
        {
            let Some((one, other, zone)) = binder.piles.take() else { return unsupported("piles: none were made before one is put somewhere"); };
            if optional || !durations.is_empty() || !act.mods.is_empty() || !controller_only(clause.subject.as_ref()) { return Err(not_lowered(&act.action)); }
            let parts = [(one, first), (other, second)].into_iter().map(|(src,to)|
                destination(to,zone,support,&binder).map(|to|(src,to))).collect::<Res<Vec<_>>>()?;
            e.instrs.push(Instr::MoveTogether { parts, entered: Vec::new() });
            e.end_act(true, optional_branch);
            continue;
        }
        // "If it's a land card, put it into your graveyard and repeat this process": "this
        // process" is the ability's instructions before it, from its start (608.2c)
        if let Action::Repeat(how) = &act.action {
            if *how != ir::procedure::Repeats::Process { return unsupported("repeat this process: a number of times, any number or until is not lowered"); }
            if branches.len() != 1 || optional_groups != 0 || !durations.is_empty() || binder.each.is_some() || !after_turns.is_empty()
                || !act.mods.is_empty() || act.may || clause.may || !controller_only(clause.subject.as_ref()) {
                return unsupported("repeat this process: only the controller's, at the end of one condition's block");
            }
            if !e.targets.is_empty() { return unsupported("repeat this process: targets are chosen once, not each time"); }
            repeated_process(&e.instrs)?;
            e.instrs.push(Instr::Repeat { from: 0, locals: (initial.len()..e.next).collect() });
            e.end_act(false, optional_branch);
            continue;
        }
        // an act no instruction does is named before its subject is asked
        if !matches!(act.action, Action::Choose(_) | Action::Reveal(_) | Action::LookAt(_) | Action::Exile(_) | Action::Move { from: None, .. }) { return Err(not_lowered(&act.action)); }
        // the controller, or the one player the clause is said of, who reveals or looks at the
        // top of "their library" and puts what the text says where it says ("defending player
        // reveals the top card of their library. If it's a land card, that player puts it into
        // their hand.", 608.2d)
        let doer = if controller_only(clause.subject.as_ref()) { binder.actor.map_or(Who::Controller, Who::Of) } else {
            let optional = (clause.may && optional_groups == 0) || act.may;
            let retained_choice = matches!(&act.action, Action::Choose(ir::action::Choice::Objects(objects))
                if matches!(objects.items.as_slice(), [selector] if mention(selector).is_some_and(|m|
                    m.taking == Taking::Exactly(1) && m.set == ObjectSet::of(Domain::Card))));
            if optional || binder.actor.is_some() || binder.each.is_some() || !single_actor(clause.subject.as_ref())
                || matches!(act.action, Action::Choose(_)) && !retained_choice {
                return unsupported("program: instruction subject not lowered");
            }
            e.actor(clause, support, &mut binder)?
        };
        // A partition allocates members of the same fixed search result before either
        // movement. Selecting identities is not an optional choice of destination: even
        // when entry is impossible, the allocated member is not reassigned (609.3).
        if let Action::Move { moves: parts, from: None, .. } = &act.action
            && let [(first, Destination::Battlefield(entries)), (second, Destination::Hand(Whose::You | Whose::Owner | Whose::Owners))] = parts.as_slice()
            && let ([first], [second]) = (first.items.as_slice(), second.items.as_slice())
            && let (Some(first), Some(second)) = (mention(first), mention(second))
            && first.taking == Taking::Exactly(1) && second.taking == Taking::Remainder
            && first.reference == UnresolvedRef::cards(RefForm::Anaphor)
            && second.reference == UnresolvedRef::cards(RefForm::Remainder)
            && first.set == ObjectSet::of(Domain::Card) && second.set == ObjectSet::of(Domain::Card)
            && let Some((source, _, max)) = binder.result.as_ref()
            && binder.searched == Some(*source)
        {
            if optional || !durations.is_empty() || !act.mods.is_empty() || binder.each.is_some()
                || *max > 2 || binder.searched_zone != Some(Zone::Library) {
                return unsupported("search partition: one mandatory library result of at most two cards required");
            }
            let source = *source;
            let to = battlefield_destination(entries, support, &binder)?;
            let set = expr::Cards::compile(&ObjectSet::of(Domain::Card), support, &mut binder.scope(support))?;
            let (found, one, other, subjects, entered) = (e.slot(), e.slot(), e.slot(), e.slot(), e.slot());
            e.instrs.push(Instr::Gather { objects: action::Operand::Reference(source), out: found });
            e.instrs.push(Instr::Select { from: found, set: set.into(), pick: Pick::Exactly(1),
                chooser: doer, gate: None, still_in: Zone::Library, out: one });
            e.instrs.push(Instr::Remainder { of: found, minus: vec![one], still_in: Zone::Library, out: other });
            e.instrs.push(Instr::Subjects { cards: one, out: subjects, still_in: None });
            e.instrs.push(Instr::MoveTogether { parts: vec![(subjects,Dest::Battlefield(to)),(other,Dest::Hand)], entered: vec![(subjects,entered)] });
            binder.entered.push(entered);
            binder.unrepresented_movement = true;
            binder.found_waiting = false;
            e.end_act(true, optional_branch);
            continue;
        }
        match &act.action {
            Action::Choose(ir::action::Choice::Objects(objects)) => {
                let [selector] = objects.items.as_slice() else { return unsupported("choice: multiple object groups not lowered"); };
                let mention = mention(selector).ok_or_else(|| crate::Unsupported("choice: no bound card group".into()))?;
                let set = binder.selection(&mention.set, support)?;
                e.unite(&mention.reference, &mut binder);
                let group = binder.resolve(&mention.reference)?;
                if !controller_only(clause.subject.as_ref()) && group.by != Verb::Reveal {
                    return unsupported("retained choice by another player: a public reveal producer required");
                }
                let (from, zone) = (group.slot, group.zone);
                let (pick, max) = match mention.taking {
                    Taking::Exactly(n) => (Pick::Exactly(n), n as u32),
                    Taking::UpTo(n) => (Pick::UpTo(n), n as u32),
                    Taking::Quantity(_) => return unsupported("variable explicit choice not lowered"),
                    Taking::All => (Pick::All, u32::MAX),
                    Taking::AnyNumber => (Pick::AnyNumber, u32::MAX),
                    Taking::Remainder => return unsupported("choice: a remainder is not a selection"),
                };
                let out = e.slot();
                e.instrs.push(Instr::Select { from, set: set.into(), pick, chooser: doer, gate: None, still_in: zone, out });
                binder.resolve(&mention.reference)?.taken.push(out);
                let subjects = e.slot();
                e.instrs.push(Instr::Subjects { cards: out, out: subjects, still_in: None });
                // An explicit choice is the result later anaphors name, including an
                // empty choice. An earlier singleton reveal/exile must not shadow it.
                binder.result = Some((subjects, mention.set.clone(), max));
                binder.name(subjects, mention.set, (0, max), false);
                binder.subjects.last_mut().expect("explicit choice just named").explicit_choice = true;
            }
            Action::Choose(choice) => {
                let kind = scalar_choice(choice)?;
                let out = e.slot();
                e.instrs.push(Instr::Choose { kind, who: Who::Controller, out });
                binder.scalars.push((out, kind.kind));
            }
            // "Exile the rest", "exile one of them": of a group the text took, as a move is
            Action::Exile(objects) if matches!(objects.items.as_slice(), [s] if mention(s).is_some()) => {
                moves = true;
                let optional = (clause.may && optional_groups == 0) || act.may;
                if optional { return unsupported("\"you may\" exile part of a group"); }
                let mut mention = mention(&objects.items[0]).expect("a mention");
                let other = mention.set.other;
                mention.set.other = false;
                let set = binder.selection(&mention.set, support)?;
                e.unite(&mention.reference, &mut binder);
                let group = binder.resolve(&mention.reference)?;
                let (mut from, zone, looked) = (group.slot, group.zone, group.by == Verb::LookAt);
                if other {
                    if group.taken.is_empty() { return unsupported("other result cards: no distinguished cards"); }
                    let remaining = e.slot();
                    e.instrs.push(Instr::Remainder { of: from, minus: group.taken.clone(), still_in: zone, out: remaining });
                    from = remaining;
                }
                let out = e.slot();
                match mention.taking {
                    Taking::AnyNumber => e.instrs.push(Instr::Select { from, set: set.into(), pick: Pick::AnyNumber, chooser: doer, gate: None, still_in: zone, out }),
                    Taking::All => e.instrs.push(Instr::Select { from, set: set.into(), pick: Pick::All, chooser: doer, gate: None, still_in: zone, out }),
                    Taking::Exactly(n) => e.instrs.push(Instr::Select { from, set: set.into(), pick: Pick::Exactly(n), chooser: doer, gate: None, still_in: zone, out }),
                    Taking::UpTo(n) => e.instrs.push(Instr::Select { from, set: set.into(), pick: Pick::UpTo(n), chooser: doer, gate: None, still_in: zone, out }),
                    Taking::Quantity(_) => return unsupported("variable selected exile not lowered"),
                    Taking::Remainder => {
                        let minus = binder.resolve(&mention.reference)?.taken.clone();
                        e.instrs.push(Instr::Remainder { of: from, minus, still_in: zone, out });
                    }
                }
                // face down, the player who looked at them may go on looking (406.3)
                let hidden = match (face_down, looked) {
                    (false, _) => None,
                    (true, true) => Some(Hide { lookers: vec![doer], watched: support.hideaway() }),
                    (true, false) => return unsupported("exile face down: of cards not looked at"),
                };
                binder.resolve(&mention.reference)?.taken.push(out);
                let shown = e.slot();
                e.instrs.push(Instr::Exile { from: out, out: shown, hidden });
                let now = e.slot();
                e.instrs.push(Instr::Subjects { cards: shown, out: now, still_in: Some(Zone::Exile) });
                binder.exiled_now.push(now);
            }
            Action::Reveal(objects) | Action::LookAt(objects) | Action::Exile(objects) => {
                let [selector] = objects.items.as_slice() else { return unsupported("program: multiple revealed groups not lowered"); };
                let (n,owner) = top_of_library(selector).ok_or_else(|| crate::Unsupported("program: reveal/exile other than library top not lowered".into()))?;
                let max = if let Amount::Const(n) = n { *n } else { u32::MAX };
                let n = quantity(n, support, &binder)?;
                if !matches!(owner.quant,Quant::Definite(_) | Quant::Target(Count::Exactly(Amount::Const(1)))) {
                    return unsupported("library slice: one bound or targeted player required");
                }
                let whose=e.operand(owner,support,&mut binder)?;
                let (mut taken, shown) = (e.slot(), e.slot());
                e.instrs.push(Instr::TakeTop { n: n.clone(), whose, out: taken });
                if optional {
                    let set = binder.selection(&ObjectSet::of(Domain::Card), support)?;
                    let (paid, possible, selected) = (e.slot(), e.slot(), e.slot());
                    binder.outcome = Some((action::Operand::Controller, paid, possible, e.instrs.len()));
                    e.instrs.push(Instr::Gate { who: Who::Controller, probe: Some(Probe { from: taken, set: set.clone(), count: n, still_in: Zone::Library }), out: paid, possible: Some(possible) });
                    e.instrs.push(Instr::Select { from: taken, set: set.into(), pick: Pick::All, chooser: Who::Controller, gate: Some(paid), still_in: Zone::Library, out: selected });
                    taken = selected;
                }
                let (by, zone) = if matches!(act.action, Action::Exile(_)) {
                    moves = true;
                    // face down, nobody has looked at them (406.3)
                    e.instrs.push(Instr::Exile { from: taken, out: shown, hidden: face_down.then(|| Hide { lookers: Vec::new(), watched: false }) });
                    (Verb::Exile, Zone::Exile)
                } else if matches!(act.action, Action::LookAt(_)) {
                    // the player alone is shown them; what they saw is theirs whether or not
                    // a choice follows ([`crate::observation`])
                    e.instrs.push(Instr::Show { from: taken, by: doer, to: Audience::Only(doer), out: shown });
                    (Verb::LookAt, Zone::Library)
                } else {
                    e.instrs.push(Instr::Show { from: taken, by: doer, to: Audience::Everyone, out: shown });
                    (Verb::Reveal, Zone::Library)
                };
                binder.groups.push(Group { slot: shown, paragraph: binder.paragraph(), by, zone, taken: Vec::new() });
                binder.produced.push(Produced { slot: shown, verb: by, shape: expr::Shape::Cards, yours: controller_only(clause.subject.as_ref()), into: (by == Verb::Exile).then_some(Zone::Exile) });
                let subjects = e.slot();
                e.instrs.push(Instr::Subjects { cards: shown, out: subjects, still_in: None });
                if by == Verb::Exile { binder.exiled_now.push(subjects); }
                binder.result = Some((subjects, ObjectSet::of(Domain::Card), max));
            }
            Action::Move { moves: parts, from: None, .. } => {
                moves = true;
                let optional = (clause.may && optional_groups == 0) || act.may;
                if optional && !(alone && parts.len() == 1) {
                    return unsupported("\"you may\" said of several moves");
                }
                // the player who does the clause chooses what is put, and puts it
                let mut gate = None;
                let mut together = Vec::new();
                let mut partition_entries = Vec::new();
                for (objects, to) in parts {
                    let [selector] = objects.items.as_slice() else { return unsupported("program: multiple moved groups not lowered"); };
                    if mention(selector).is_none() && !optional && parts.len() > 1
                        && let Some((cards, zone)) = e.retained_selection(selector, support, &mut binder)? {
                        let selected = e.slot();
                        e.instrs.push(Instr::Remainder { of: cards, minus: Vec::new(), still_in: zone, out: selected });
                        let to = destination(to, zone, support, &binder)?;
                        let into = match to { Dest::Hand => Zone::Hand, Dest::Graveyard => Zone::Graveyard, Dest::Library { .. } => Zone::Library, Dest::Battlefield(_) => unreachable!("entry is refused by destination") };
                        binder.produced.push(Produced { slot: selected, verb: Verb::Move, shape: expr::Shape::Cards, yours: doer == Who::Controller, into: Some(into) });
                        together.push((selected, to));
                        continue;
                    }
                    let mention = mention(selector).ok_or_else(|| crate::Unsupported("program: non-result movement not lowered".into()))?;
                    let set = binder.selection(&mention.set, support)?;
                    e.unite(&mention.reference, &mut binder);
                    let group = binder.resolve(&mention.reference)?;
                    let (from, zone) = (group.slot, group.zone);
                    if optional && mention.taking == Taking::Remainder {
                        return unsupported("\"you may\" put the rest");
                    }
                    if optional && mention.taking != Taking::AnyNumber {
                        let probe = match &mention.taking {
                            Taking::Exactly(count) => Some(Probe { from, set: set.clone(), count: expr::Amount::constant(*count as u32), still_in: zone }),
                            Taking::Quantity(_) => return unsupported("optional variable result movement not lowered"),
                            _ => None,
                        };
                        let out = e.slot();
                        e.instrs.push(Instr::Gate { who: doer, probe, out, possible: None });
                        gate = Some(out);
                    }
                    let out = e.slot();
                    match mention.taking {
                        Taking::AnyNumber => e.instrs.push(Instr::Select { from, set: set.clone().into(), pick: Pick::AnyNumber, chooser: doer, gate, still_in: zone, out }),
                        Taking::All => e.instrs.push(Instr::Select { from, set: set.clone().into(), pick: Pick::All, chooser: doer, gate, still_in: zone, out }),
                        Taking::Exactly(n) => e.instrs.push(Instr::Select { from, set: set.clone().into(), pick: Pick::Exactly(n), chooser: doer, gate, still_in: zone, out }),
                        Taking::UpTo(n) => e.instrs.push(Instr::Select { from, set: set.clone().into(), pick: Pick::UpTo(n), chooser: doer, gate, still_in: zone, out }),
                        Taking::Quantity(n) => {
                            let n = quantity(&n, support, &binder)?;
                            e.instrs.push(Instr::Select { from, set: set.clone().into(), pick: Pick::Quantity(n), chooser: doer, gate, still_in: zone, out });
                        },
                        Taking::Remainder => {
                            let minus = binder.resolve(&mention.reference)?.taken.clone();
                            e.instrs.push(Instr::Remainder { of: from, minus, still_in: zone, out });
                        }
                    }
                    binder.resolve(&mention.reference)?.taken.push(out);
                    match to {
                        Destination::Battlefield(entries) => {
                            let to = battlefield_destination(entries, support, &binder)?;
                            let subjects = e.slot();
                            e.instrs.push(Instr::Subjects { cards: out, out: subjects, still_in: None });
                            let entered = e.slot();
                            binder.entered.push(entered);
                            binder.produced.push(Produced { slot: entered, verb: Verb::Move, shape: expr::Shape::Moved, yours: doer == Who::Controller, into: Some(Zone::Battlefield) });
                            if parts.len() > 1 {
                                together.push((subjects,Dest::Battlefield(to)));
                                partition_entries.push((subjects,entered));
                            } else {
                                e.instrs.push(Instr::Action { action: action::Action::Move { groups: vec![(vec![action::Operand::Reference(subjects)], to)], said: ir::action::MoveVerb::Put }, created: Some(entered), attempt: None });
                            }
                        }
                        _ => {
                            let to = destination(to, zone, support, &binder)?;
                            // what was chosen to be put there, as it was chosen (608.2h)
                            let into = match to { Dest::Hand => Zone::Hand, Dest::Graveyard => Zone::Graveyard, Dest::Library { .. } => Zone::Library, Dest::Battlefield(_) => unreachable!("entry handled above") };
                            binder.produced.push(Produced { slot: out, verb: Verb::Move, shape: expr::Shape::Cards, yours: doer == Who::Controller, into: Some(into) });
                            if parts.len() > 1 { together.push((out,to)); }
                            else {e.instrs.push(Instr::Move { src: out, to, by: doer, when: None });}
                        }
                    }
                }
                if !together.is_empty() {e.instrs.push(Instr::MoveTogether { parts:together, entered:partition_entries });}
            }
            other => return Err(not_lowered(other)),
        }
        e.end_act(moves, optional_branch);
    }
    reach.at = Some(acts.len());
    // 601.2c/608.2b: all announced targets are available on either conditional path.
    // Captures have no effects and are checked once before following the body.
    let is_target = |i: &Instr| matches!(i, Instr::Capture { operand: action::Operand::Targets(_), .. })
        || matches!(i, Instr::Capture { out, .. } if e.target_relations.contains(out));
    let count = e.instrs.iter().filter(|i| is_target(i)).count();
    if count > 0 {
        let mut positions = vec![0; e.instrs.len() + 1];
        let mut next = count;
        for (pc, i) in e.instrs.iter().enumerate() { positions[pc] = next; if !is_target(i) { next += 1; } }
        positions[e.instrs.len()] = next;
        let mut instrs: Vec<_> = e.instrs.iter().filter(|i| is_target(i)).cloned().collect();
        instrs.extend(e.instrs.into_iter().filter(|i| !is_target(i)).map(|mut i| {
            if let Instr::Branch { end, .. } = &mut i { *end = positions[*end]; }
            if let Instr::Repeat { from, .. } = &mut i { *from = positions[*from]; }
            i
        }));
        // Optional offsets also shrink when captures move out of their block.
        for (pc, i) in instrs.iter_mut().enumerate() {
            if let Instr::SkipUnless { count: skip, .. } = i {
                let end = e.acts.iter().find(|a| positions[a.end] > pc).expect("optional act").end;
                *skip = positions[end] - pc - 1;
            }
        }
        e.acts.iter_mut().for_each(|a| a.end = positions[a.end]);
        e.acts.insert(0, ActSpan { end: count, moves: false, keeps_x: false });
        e.instrs = instrs;
    }
    if body_x { for act in &mut e.acts { act.keeps_x = true; } }
    let mut program = Program::with_initial(e.instrs, e.acts, e.targets, initial)?;
    program.announcement = crate::announcement::Facts::compile(effect, support)?;
    if crate::ability_limit::Limit::of(effect)? == Some(crate::ability_limit::Limit::Action) {
        let producers = program.instrs.iter().filter(|i| matches!(i, Instr::Pay { .. } | Instr::Cast { .. })).count();
        let options: Vec<_> = acts.iter().filter_map(|step| match step {
            TextStep::Act(clause, act, _) if clause.may || act.may => Some((*clause, *act)), _ => None,
        }).collect();
        let identified = matches!(options.as_slice(), [(clause, act)] if controller_only(clause.subject.as_ref())
            && matches!(act.action, Action::PayMana(_) | Action::PayLife(_) | Action::Cast { .. }));
        if producers != 1 || !identified { return unsupported("action limit: requires one identified controller payment or cast"); }
        program.action_limit = true;
    }
    Ok(program.sealed())
}

/// Explicit compilation tool entry point. No runtime execution falls back to this call.
impl Body {
    /// Of one refused, what its lowering never reached is left unreached ([`Lowering`]): all
    /// its instructions and its modes' when it refused before lowering any.
    fn compile(effect: &Effect, modes: &[ir::Mode], support: &Support<'_>) -> Res<Self> {
        let entered = support.lowering().get().entered;
        let body = Self::lowered(effect, modes, support);
        if body.is_err() && support.lowering().get().entered == entered {
            unreached(support, instructions(&effect.0) + modes.iter().filter_map(crate::modes::effect_of).map(|e| instructions(&e.0)).sum::<usize>());
        }
        body
    }
    fn lowered(effect: &Effect, modes: &[ir::Mode], support: &Support<'_>) -> Res<Self> {
        let body=match crate::modes::bounds(effect, modes)? {
            None => lower(effect, support).map(Arc::new).map(Self::Plain),
            Some(crate::modes::Bounds { min, max, instead, paws }) => {
                let instead = match instead {
                    None => None,
                    // of a spell, as announced (601.2b)
                    Some((condition, a, b)) if matches!(support.ability(), Some(ir::Ability::Text(_))) => Some((match condition {
                        ir::Condition::CastWas { what, fact: ir::condition::CastFact::Kicked, .. }
                            if what.quant == ir::Quant::Definite(ir::Ref::This) && support.has_kicker() => Unlock::Kicked,
                        ir::Condition::CastWas { what, fact, .. } if what.quant == ir::Quant::Definite(ir::Ref::This)
                            && support.optional().is_some_and(|(kind, cost)| crate::program::condition::optional_paid(fact, kind, cost)) => Unlock::Optional,
                        _ => return unsupported("modes: chosen instead as something other than its announcement says"),
                    }, a, b)),
                    Some(_) => return unsupported("modes: chosen instead, of an ability that is not a spell"),
                };
                // a mode's own cost is paid as its spell is cast (700.2h)
                if modes.iter().any(|m| matches!(m, ir::Mode::Costed { .. })) && !matches!(support.ability(), Some(ir::Ability::Text(_) | ir::Ability::Keywords(_))) {
                    return unsupported("modes: a mode with its own cost, of an ability that is not a spell (700.2h)");
                }
                // in printed order, stopping at the first mode refused: the modes after it
                // are not reached
                let effects: Vec<_> = modes.iter().map(|mode| crate::modes::effect_of(mode).expect("mode bounds validates mode forms")).collect();
                let mut programs = Vec::new();
                for (m, effect) in effects.iter().enumerate() {
                    match lower(effect, support) {
                        Ok(program) => programs.push(Arc::new(program)),
                        Err(why) => {
                            unreached(support, effects[m + 1..].iter().map(|e| instructions(&e.0)).sum());
                            return Err(why);
                        }
                    }
                }
                let paws = paws.map(|most| (most, (0..modes.len()).map(|m| crate::modes::pawprints(modes, &[m])).collect()));
                Ok(Self::Modes { min, max, programs, repeats: crate::modes::repeats(effect), instead, paws })
            }
        }?;
        // Mana resolutions have no stack object: do not pretend their permanent
        // source is a unique shield occurrence. Mixed modal bodies are bounded too.
        let programs=body.programs();
        if matches!(support.ability(),Some(ir::Ability::Activated{..}|ir::Ability::Triggered{..}))
            && programs.iter().any(|p|p.announcement.adds_mana)
            && programs.iter().any(|p|p.instrs.iter().any(|i|matches!(i,Instr::Action{action:action::Action::Prevent{..},..}))) {
            return unsupported("prevention combined with mana in an ability needs an explicit nonstack occurrence identity");
        }
        Ok(body)
    }
}

pub(crate) fn compile_definition(def: &crate::card::CardDef) -> Vec<CompiledEffect> {
    let mut compiled = Vec::new();
    for (i, ability) in def.abilities.iter().chain(def.keyword_abilities.iter().map(|(_, a)| a)).enumerate() {
        // of an opening-hand action, what taking it has its player do (103.6)
        let follow_up;
        let header;
        let effect = match ability {
            _ if i < def.abilities.len() && def.opening.contains(&i) => match crate::start::follow_up(ability) {
                Some(effect) => { follow_up = effect; &follow_up }
                None => continue,
            },
            // Spree: "Choose one or more —", Tiered: "Choose one —" of the costed modes
            // printed under it (702.172a, 702.183a)
            ir::Ability::Keywords(_) => match crate::keyword::modal_row(def) {
                Some((row, modes)) if row == i => {
                    header = Effect(vec![ir::Sentence::ChooseModes { modes: modes.clone(), unchosen: None, random: false, chooser: None }]);
                    &header
                }
                _ => continue,
            },
            ir::Ability::Text(e) | ir::Ability::Triggered { effect: e, .. }
                | ir::Ability::Activated { effect: e, .. } | ir::Ability::Chapter { effect: e, .. } => e,
            _ => continue,
        };
        // a paragraph that replaces the one before it is lowered with it, as its last sentence;
        // a spell's paragraph after another, as its later sentences
        let printed = |at: usize| def.abilities.get(at).filter(|_| i < def.abilities.len());
        if i > 0 && matches!(printed(i - 1), Some(ir::Ability::Text(_))) && replaces_previous(ability).is_some() { continue; }
        if i < def.abilities.len() && continues_previous(def, i) { continue; }
        let last = if i < def.abilities.len() { (i + 1..).take_while(|j| continues_previous(def, *j)).last().unwrap_or(i) } else { i };
        let joined;
        let effect = if last > i {
            joined = Effect(effect.0.iter().chain((i + 1..=last).flat_map(|j| match &def.abilities[j] {
                ir::Ability::Text(e) => e.0.iter(),
                _ => unreachable!("a continuing paragraph is text"),
            })).cloned().collect());
            &joined
        } else { effect };
        let replaced;
        let effect = match printed(last + 1).and_then(replaces_previous) {
            Some(instead) if matches!(ability, ir::Ability::Text(_)) => {
                replaced = Effect(effect.0.iter().cloned().chain([instead]).collect());
                &replaced
            }
            _ => effect,
        };
        let support = Support::definition(def, Some(crate::binding::AbilityKey::printed(i)));
        let result = Body::compile(effect, def.modes_of(i), &support);
        let linked_exile_writers = if result.is_ok() { support.linked_exile_writers() } else { Vec::new() };
        compiled.push(CompiledEffect {
            ability: i,
            linked_exile_writers,
            unreached: if result.is_err() { support.lowering().get().unreached } else { 0 },
            result,
            intervening: crate::trigger::intervening_if(ability).map(|c| {
                let support = Support::definition(def, Some(crate::binding::AbilityKey::printed(i)));
                initial_condition(c, &support)
            }).transpose(),
            triggered: matches!(ability, ir::Ability::Triggered { .. } | ir::Ability::Chapter { .. }),
            chapter: matches!(ability, ir::Ability::Chapter { .. }),
        });
    }
    compiled
}

