//! Combat (CR 506–511): the active player declares attackers, each defending player declares
//! blockers, and combat damage is assigned and dealt, each as the turn-based action of its step
//! (703.4i–m). Attacking and blocking are actions of the lexicon: who can do them and what can
//! be attacked is asked of their entries (506.3), as for any other action.

use crate::event::{Event, Target};
use crate::state::{Decision, Game, ObjId, PlayerId};
use ir::{CardType, Role, Verb};

/// What a combat phase has so far. Emptied as the end of combat step ends (511.3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Combat {
    /// Creatures were declared as attackers this combat (508.8), whatever became of them.
    pub declared: bool,
    /// The attacking creatures and what each is attacking (508.1k), in the order declared.
    pub attackers: Vec<(ObjId, Target)>,
    /// The blocking creatures and the attacking creature each is blocking (509.1g).
    pub blockers: Vec<(ObjId, ObjId)>,
    /// The blocking creatures: one remains a blocking creature until it is removed from
    /// combat or the combat phase ends (509.1g), whatever becomes of what it blocks.
    pub blocking: Vec<ObjId>,
    /// The attacking creatures that became blocked; one stays blocked even if what blocked it
    /// leaves combat (509.1h).
    pub blocked: Vec<ObjId>,
    /// Blockers have been declared: an attacking creature that is not blocked is unblocked.
    pub blockers_declared: bool,
    /// The defending players still to declare blockers, in turn order (101.4).
    pub(crate) to_block: Vec<PlayerId>,
    /// The combat damage assigned so far (510.1): its source, what it is dealt to, how much.
    pub(crate) assigned: Vec<(ObjId, Target, u32)>,
    /// Attacking/blocking creatures whose damage needs division among recipients (510.1c–d).
    pub(crate) to_divide: Vec<ObjId>,
    /// How many combat damage steps have begun this combat: a second follows the first when a
    /// creature in it had first strike or double strike (510.4).
    pub(crate) damage_steps: u8,
    /// The creatures that had first strike or double strike as the first combat damage step
    /// began, when any had (510.4).
    pub(crate) first_strikers: Option<Vec<ObjId>>,
    /// The creatures that assign combat damage in this combat damage step (510.1a, 510.4).
    pub(crate) assigning: Vec<ObjId>,
    /// The creatures declared as attackers (508.1), whatever has since become of them or
    /// entered attacking (506.5, 508.3a, 508.4): what attack-count triggers count.
    pub(crate) declared_attackers: Vec<ObjId>,
    /// The attacking creatures removed from combat, and what each was attacking then (508.5).
    pub(crate) removed: Vec<(ObjId, Target)>,
}

/// Declaration is a retained rules action, not priority or a stack object (508.1f–m).
/// Object collection is conservatively suspended while any procedure remains active.
#[derive(Debug, Clone)]
pub(crate) struct Declaration {
    attacks: Vec<(ObjId, Target)>,
    tapped: Vec<ObjId>,
    pub(crate) from: crate::event::Seq,
    triggers: std::ops::Range<usize>,
    /// The turn uses of the abilities the declaration taps triggered: given back with them.
    uses: Vec<crate::ability_limit::Use>,
    /// What triggered while it waited for payment but for one of those uses, the first for
    /// each: what triggers after all if the declaration is reversed (733.1).
    passed: Vec<(crate::ability_limit::Use, crate::state::Triggered)>,
    cost: ir::ManaCost,
    pub(crate) answer: Option<crate::state::Answer>,
    pub(crate) paid: Option<bool>,
}

impl Declaration {
    pub(crate) fn passed_over(&mut self, used: crate::ability_limit::Use, trigger: &crate::state::Triggered) {
        if self.uses.contains(&used) && !self.passed.iter().any(|(u, _)| *u == used) { self.passed.push((used, trigger.clone())); }
    }
}

impl Combat {
    pub fn attacking(&self, o: ObjId) -> bool {
        self.attackers.iter().any(|(a, _)| *a == o)
    }

    pub fn blocking(&self, o: ObjId) -> bool {
        self.blocking.contains(&o)
    }

    /// The creatures blocking an attacking creature.
    pub fn blockers_of(&self, attacker: ObjId) -> Vec<ObjId> {
        self.blockers.iter().filter(|(_, a)| *a == attacker).map(|(b, _)| *b).collect()
    }

    /// A permanent is removed from combat (506.4): it stops being an attacking or blocking
    /// creature. A creature it blocked remains blocked, and a creature that blocked it
    /// remains a blocking creature, blocking nothing.
    pub(crate) fn remove(&mut self, o: ObjId) {
        self.removed.extend(self.attackers.iter().filter(|(a, _)| *a == o).copied());
        self.attackers.retain(|(a, _)| *a != o);
        self.blockers.retain(|(b, a)| *b != o && *a != o);
        self.blocking.retain(|b| *b != o);
        self.blocked.retain(|a| *a != o);
        self.assigned.retain(|(source, _, _)| *source != o);
        self.to_divide.retain(|a| *a != o);
        self.assigning.retain(|a| *a != o);
    }
}

impl Game {
    /// An attacking or blocking creature that stops being a creature, or becomes a battle,
    /// is removed from combat immediately (506.4), including during resolution. This is not
    /// a state-based action. Ask the type layer outside its own computation: combat status
    /// may itself determine which continuous effects apply. Removing combatants can change
    /// those effects, so continue until no further removal is needed.
    pub(crate) fn update_combat(&mut self) {
        loop {
            let removed: Vec<ObjId> = self.combat.attackers.iter().map(|(a, _)| *a)
                .chain(self.combat.blocking.iter().copied())
                .filter(|o| {
                    let seen = self.seen(*o, Some(crate::layers::Layer::Colors));
                    !seen.is(CardType::Creature) || seen.is(CardType::Battle)
                })
                .collect();
            if removed.is_empty() {
                break;
            }
            for o in removed {
                self.combat.remove(o);
            }
        }
    }

    /// The player a creature attacks when it attacks this: the player, or the planeswalker's
    /// controller (506.2, 508.5).
    /// Is this still being attacked? A planeswalker or battle that leaves the battlefield, or
    /// stops being one, is removed from combat; what attacked it is still attacking, but
    /// attacks nothing (506.4c).
    pub(crate) fn still_attacked(&self, attacked: Target) -> bool {
        match attacked {
            Target::Player(p) => !self.players[p].lost,
            Target::Object(o) => self.obj(o).zone == crate::Zone::Battlefield
                && (self.is_a(o, CardType::Planeswalker) || self.is_a(o, CardType::Battle)),
        }
    }

    /// The defending player an attacking creature's ability refers to (508.5): the player it
    /// is attacking, or the controller of the planeswalker it is attacking; one removed from
    /// combat, the one it was attacking before. None for a creature that has not attacked.
    pub(crate) fn defending_player_of(&self, creature: ObjId) -> Option<PlayerId> {
        self.combat.attackers.iter().chain(self.combat.removed.iter().rev())
            .find(|(a, _)| *a == creature).map(|(_, attacked)| self.defending(*attacked))
    }

    pub(crate) fn defending(&self, attacked: Target) -> PlayerId {
        match attacked {
            Target::Player(p) => p,
            Target::Object(o) => self.controller(o),
        }
    }

    /// Can this creature be declared as an attacker now (508.1a)? It is a creature (506.3)
    /// the active player controls, untapped, not also a battle, and under that player's control
    /// continuously since their most recent turn began (302.6).
    pub fn can_attack(&self, creature: ObjId) -> Result<(), String> {
        let o = self.obj(creature);
        if !Verb::Attack.performer().iter().any(|s| self.is(Target::Object(creature), *s)) {
            return Err("only a creature can attack (506.3)".into());
        }
        if self.controller(creature) != self.active {
            return Err("only creatures the active player controls can attack (508.1a)".into());
        }
        if o.tapped {
            return Err("a tapped creature can't be declared as an attacker (508.1a)".into());
        }
        if self.is_a(o.id, CardType::Battle) {
            return Err("a creature that is also a battle can't attack (508.1a)".into());
        }
        if self.summoning_sick(creature) {
            return Err("that creature has not been under your control since your turn began (302.6)".into());
        }
        Ok(())
    }

    /// Can this be attacked (506.3, 508.1b)? A player other than the attacking player, or a
    /// planeswalker such a player controls. (A battle is attacked through its protector, 310.8,
    /// which the engine does not have yet.)
    fn can_be_attacked(&self, target: Target) -> Result<(), String> {
        if !self.takes(Verb::Attack, Role::To, target) {
            return Err("only a player, a planeswalker or a battle can be attacked (506.3)".into());
        }
        if matches!(target, Target::Object(o) if !self.is_a(o, CardType::Planeswalker)) {
            return Err("attacking a battle is not in the engine yet".into());
        }
        if self.defending(target) == self.active {
            return Err("a creature attacks a defending player or their planeswalker (506.2)".into());
        }
        Ok(())
    }

    /// Entry attacking names its defender without declaring an attack (508.4, 702.49c).
    /// A departed defender does not turn into its controller or its successor (508.4a).
    pub(crate) fn enter_attacking(&mut self, creature: ObjId, defender: Target) {
        if !self.is_a(creature, CardType::Creature) || self.is_a(creature, CardType::Battle) { return; }
        if !self.entering_attacking(self.controller(creature)) { return; }
        if matches!(defender, Target::Object(o) if self.get_object(o).is_none_or(|o| o.zone != crate::Zone::Battlefield)) { return; }
        if self.can_be_attacked(defender).is_err() { return; }
        self.combat.attackers.push((creature, defender));
    }

    /// Can this creature be declared as a blocker by this player (509.1a)? It is a creature
    /// (506.3) that player controls, untapped and not also a battle.
    pub fn can_block(&self, player: PlayerId, creature: ObjId) -> Result<(), String> {
        let o = self.obj(creature);
        if !Verb::Block.performer().iter().any(|s| self.is(Target::Object(creature), *s)) {
            return Err("only a creature can block (506.3)".into());
        }
        if self.controller(creature) != player {
            return Err("a player declares only creatures they control as blockers (509.1a)".into());
        }
        if o.tapped {
            return Err("a tapped creature can't be declared as a blocker (509.1a)".into());
        }
        if self.is_a(o.id, CardType::Battle) {
            return Err("a creature that is also a battle can't block (509.1a)".into());
        }
        Ok(())
    }

    /// Can a creature this player controls be put onto the battlefield attacking? Only an
    /// attacking player's, once attackers are declared until combat ends (506.3b, 508.4).
    pub(crate) fn entering_attacking(&self, player: PlayerId) -> bool {
        player == self.active && matches!(self.part, ir::Part::DeclareAttackers | ir::Part::DeclareBlockers | ir::Part::CombatDamage | ir::Part::EndOfCombat)
    }

    /// The players a creature of the active player could attack (506.2).
    pub(crate) fn defending_players(&self) -> Vec<PlayerId> {
        (0..self.players.len()).filter(|p| *p != self.active && !self.players[*p].lost).collect()
    }

    /// Exact supported recipients, including planeswalkers but not battles/protectors.
    pub(crate) fn attack_recipients(&self) -> Vec<Target> {
        self.defending_players().into_iter().map(Target::Player)
            .chain(self.battlefield.iter().copied().map(Target::Object)
                .filter(|t| self.can_be_attacked(*t).is_ok())).collect()
    }

    pub(crate) fn required_to_attack(&self, creature: ObjId) -> bool {
        self.must_attack(creature) && self.can_attack(creature).is_ok()
            && self.attack_recipients().into_iter().any(|t| !self.cant_attack(creature, t)
                && self.attack_cost(&[(creature, t)]).0.is_empty())
    }

    /// Could this creature be declared as an attacker: it can attack (508.1a), and no effect
    /// says it can't attack every player it could (508.1c)?
    pub(crate) fn able_to_attack(&self, creature: ObjId) -> bool {
        self.can_attack(creature).is_ok()
            && self.attack_recipients().into_iter().any(|t| !self.cant_attack(creature, t))
    }

    // ---------------------------------------------------------------- declare attackers (508)

    /// The declare attackers step begins: the active player declares attackers (508.1). With
    /// no creature that could attack the declaration is empty and nobody is asked.
    pub(crate) fn begin_declare_attackers(&mut self) {
        let able = self.battlefield.iter().any(|o| self.able_to_attack(*o));
        if able {
            self.decision = Some(Decision::Attackers { player: self.active });
        }
    }

    /// The active player's declaration (508.1): which creatures attack, and what each attacks.
    /// An illegal declaration is refused whole and nothing has happened (508.1, 733).
    pub(crate) fn declare_attackers(&mut self, attacks: Vec<(ObjId, Target)>) -> Result<(), String> {
        // Validate the whole declaration before taps/payment.
        for (i, (creature, target)) in attacks.iter().enumerate() {
            if attacks[..i].iter().any(|(c, _)| c == creature) {
                return Err("a creature is declared as an attacker once".into());
            }
            self.can_attack(*creature)?;
            self.can_be_attacked(*target)?;
        }
        // restrictions: an effect that says a creature can't attack is obeyed (508.1c)
        if let Some((creature, _)) = attacks.iter().find(|(c, t)| self.cant_attack(*c, *t)) {
            return Err(format!("{} can't attack that (508.1c)", self.obj(*creature).chars.name));
        }
        // requirements: a creature that attacks if able, and is able, attacks (508.1d)
        let stays = self.battlefield.iter().find(|o| {
            self.required_to_attack(**o) && !attacks.iter().any(|(c, _)| c == *o)
        });
        if let Some(creature) = stays {
            return Err(format!("{} attacks if able (508.1d)", self.obj(*creature).chars.name));
        }
        self.decision = None;
        let mut tapped = Vec::new();
        let triggers_start = self.triggered.len();
        // the attacking creatures become tapped at once: one event (508.1f, 603.2c)
        let taps = self.log.end();
        for (creature, _) in &attacks {
            if !self.has(*creature, "Vigilance") {
                self.tap(*creature);
                tapped.push(*creature);
            }
        }
        self.finish_simultaneous(taps, crate::layers::Working::before(Vec::new()));
        // Tap-triggered effects are retained until declaration completes (508.2b).
        let used = self.ability_uses.len();
        self.check_triggers();
        let uses = self.ability_uses[used..].to_vec();
        let from = self.log.end();
        let cost = self.attack_cost(&attacks); // total locks after declaration taps (508.1h)
        if cost.0.is_empty() {
            self.complete_attack_declaration(attacks);
        } else {
            self.decision = Some(Decision::Choose { player: self.active,
                choice: crate::state::Choice::Pay { cost: cost.clone(), life: 0, waterbend: None } });
            let triggers = triggers_start..self.triggered.len();
            self.procedures.push(crate::procedure::Procedure::AttackDeclaration(Declaration {
                attacks, tapped, from, triggers, uses, passed: Vec::new(), cost,
                answer: None, paid: None,
            }));
        }
        Ok(())
    }

    fn complete_attack_declaration(&mut self, mut attacks: Vec<(ObjId, Target)>) {
        attacks.retain(|(c, _)| self.get_object(*c).is_some_and(|o| o.zone == crate::Zone::Battlefield)
            && self.controller(*c) == self.active && self.is_a(*c, CardType::Creature)
            && !self.is_a(*c, CardType::Battle));
        self.combat.declared = !attacks.is_empty();
        self.combat.attackers = attacks.clone();
        self.combat.declared_attackers = attacks.iter().map(|(a, _)| *a).collect();
        // the creatures attack at once: one event, of each of them (508.1, 603.2c)
        let from = self.log.end();
        for (attacker, target) in attacks {
            let attacked = Event::Attacked { attacker, target };
            self.note(&attacked);
            self.log.push(attacked);
        }
        if self.combat.declared { let active = self.active; self.log.push(Event::AttackersDeclared { player: active }); }
        self.finish_simultaneous(from, crate::layers::Working::before(Vec::new()));
    }

    pub(crate) fn advance_attack_declaration(&mut self) {
        use crate::procedure::Procedure;
        use crate::state::Answer;
        let Some(Procedure::AttackDeclaration(mut at)) = self.procedures.pop() else { unreachable!() };
        if at.paid == Some(true) {
            self.complete_attack_declaration(at.attacks);
            return;
        }
        if at.paid == Some(false) || matches!(at.answer, Some(Answer::No)) {
            // Restore only declaration taps; legal later mana actions keep their effects.
            for creature in at.tapped {
                let later_tap = self.log.since(at.from).iter().any(|e| matches!(e, Event::Tapped { object } if *object == creature));
                if !later_tap && self.get_object(creature).is_some_and(|o| o.zone == crate::Zone::Battlefield && o.tapped) {
                    self.obj_mut(creature).tapped = false;
                }
            }
            self.triggered.drain(at.triggers);
            // nothing triggered on the undone taps, so no turn use was spent on it (733.1);
            // what the payment really did since may then trigger those abilities after all
            self.ability_uses.retain(|u| !at.uses.contains(u));
            for (used, trigger) in at.passed {
                if !self.ability_uses.contains(&used) { self.ability_uses.push(used); self.triggered.push(trigger); }
            }
            self.decision = Some(Decision::Attackers { player: self.active });
            return;
        }
        let (steps, with) = match at.answer.take().expect("declaration payment answered") {
            Answer::Pay { plan, with, .. } => (plan, with),
            _ => unreachable!("payment choice validated"),
        };
        let payment = crate::payment::Payment::effect(self.active, at.attacks[0].0,
            crate::cost::Plan::default().kicked(&at.cost, 1), steps, Vec::new(), with);
        self.procedures.push(Procedure::AttackDeclaration(at));
        self.procedures.push(Procedure::Payment(payment));
    }

    // ----------------------------------------------------------------- declare blockers (509)

    /// The declare blockers step begins: each defending player declares blockers (509.1), in
    /// turn order from the active player (101.4).
    pub(crate) fn begin_declare_blockers(&mut self) {
        let n = self.players.len();
        let attacked: Vec<PlayerId> = self.combat.attackers.iter().map(|(_, t)| self.defending(*t)).collect();
        self.combat.to_block = (1..n).map(|i| (self.active + i) % n).filter(|p| attacked.contains(p)).collect();
        self.next_to_block();
    }

    /// Ask the next defending player who has a creature that could block; when none is left
    /// the blockers are declared.
    fn next_to_block(&mut self) {
        while let Some(player) = self.combat.to_block.first().copied() {
            let attacking_them: Vec<ObjId> =
                self.combat.attackers.iter().filter(|(_, t)| self.defending(*t) == player).map(|(a, _)| *a).collect();
            // is there a block to declare? A creature that could block one of them alone, or
            // together with every other that could ("except by three or more creatures")
            let could: Vec<ObjId> = self.battlefield.iter().copied().filter(|b| self.can_block(player, *b).is_ok()).collect();
            let able = could.iter().any(|b| {
                attacking_them.iter().any(|a| !self.cant_block(*b, *a, &[*b]) || !self.cant_block(*b, *a, &could))
            });
            // the declaration is made whenever the player controls a creature (509.1a), even when
            // no block would be legal; a harness replaying a record asks for it (`ask_forced`) so
            // that an illegal block can be attempted and refused (509.1b)
            let forced = self.ask_forced
                && self.battlefield.iter().any(|c| self.controller(*c) == player && self.is_a(*c, CardType::Creature));
            if able || forced {
                self.decision = Some(Decision::Blockers { player });
                return;
            }
            self.combat.to_block.remove(0);
        }
        self.blockers_declared();
    }

    /// A defending player's declaration (509.1): which creatures block, and which attacking
    /// creature each blocks, one that is attacking that player or a planeswalker they control.
    pub(crate) fn declare_blockers(&mut self, player: PlayerId, blocks: Vec<(ObjId, ObjId)>) -> Result<(), String> {
        for (i, (blocker, attacker)) in blocks.iter().enumerate() {
            if blocks[..i].contains(&(*blocker, *attacker)) {
                return Err("the same block cannot be declared twice (509.1a)".into());
            }
            if blocks[..i].iter().filter(|(b, _)| b == blocker).count() >= self.block_capacity(*blocker) {
                return Err("that creature cannot block that many attacking creatures (509.1a)".into());
            }
            self.can_block(player, *blocker)?;
            let attacking_them = self.combat.attackers.iter().any(|(a, t)| a == attacker && self.defending(*t) == player);
            if !attacking_them {
                return Err("a creature blocks a creature that is attacking its controller or their planeswalker (509.1a)".into());
            }
        }
        // restrictions: an effect that says a creature can't block, or can't be blocked that
        // way, is obeyed (509.1b); requirements to block (509.1c) are not in the engine yet
        for (blocker, attacker) in &blocks {
            let mut all_blocking_it = self.combat.blockers_of(*attacker);
            all_blocking_it.extend(blocks.iter().filter(|(_, a)| a == attacker).map(|(b, _)| *b));
            if let Some(restriction) = self.block_restriction(*blocker, *attacker, &all_blocking_it) {
                let (b, a) = (&self.obj(*blocker).chars.name, &self.obj(*attacker).chars.name);
                return Err(format!("{b} can't block {a}: {restriction}"));
            }
        }
        self.decision = None;
        for (blocker, _) in &blocks {
            if !self.combat.blocking.contains(blocker) {
                self.combat.blocking.push(*blocker);
            }
        }
        self.combat.blockers.extend(blocks);
        self.combat.to_block.retain(|p| *p != player);
        self.next_to_block();
        Ok(())
    }

    /// Every defending player has declared: the chosen creatures are blocking creatures
    /// (509.1g), and an attacking creature with a blocker declared for it is blocked, one with
    /// none unblocked (509.1h). Abilities that trigger on blockers being declared trigger
    /// (509.1i).
    fn blockers_declared(&mut self) {
        self.combat.blockers_declared = true;
        // the creatures block and become blocked at once: one event (509.1h-i, 603.2c)
        let from = self.log.end();
        for blocker in self.combat.blocking.clone() {
            let blocking = Event::BecameBlocking { blocker };
            self.note(&blocking);
            self.state_mut().log.push(blocking);
        }
        let attackers: Vec<ObjId> = self.combat.attackers.iter().map(|(a, _)| *a).collect();
        for attacker in attackers {
            let blockers = self.combat.blockers_of(attacker);
            if blockers.is_empty() {
                continue;
            }
            self.combat.blocked.push(attacker);
            self.log.push(Event::BecameBlocked { attacker });
            for blocker in blockers {
                self.log.push(Event::Blocked { blocker, attacker });
            }
        }
        self.finish_simultaneous(from, crate::layers::Working::before(Vec::new()));
    }

    // -------------------------------------------------------------------- combat damage (510)

    /// Does this creature have first strike or double strike (702.7, 702.4)?
    fn strikes_first(&self, creature: ObjId) -> bool {
        self.has(creature, "First strike") || self.has(creature, "Double strike")
    }

    /// The creatures that assign combat damage in the combat damage step that is beginning
    /// (510.4). If an attacking or blocking creature has first strike or double strike as the
    /// first one begins, only those do, and the phase gets a second; in that one, the
    /// remaining attackers and blockers that had neither as the first began, and those that
    /// have double strike now.
    fn assigning(&mut self) -> Vec<ObjId> {
        let c = &self.combat;
        let fighting: Vec<ObjId> = c.attackers.iter().map(|(a, _)| *a).chain(c.blocking.iter().copied()).collect();
        if let Some(first) = &c.first_strikers {
            return fighting.into_iter().filter(|o| !first.contains(o) || self.has(*o, "Double strike")).collect();
        }
        let first: Vec<ObjId> = fighting.iter().copied().filter(|o| self.strikes_first(*o)).collect();
        if first.is_empty() {
            return fighting;
        }
        self.combat.first_strikers = Some(first.clone());
        first
    }

    /// Does a second combat damage step follow this one (510.4)?
    pub(crate) fn second_combat_damage_step(&self) -> bool {
        self.combat.damage_steps == 1 && self.combat.first_strikers.is_some()
    }

    /// The combat damage step begins: who assigns combat damage in it is settled (510.4), and
    /// they do.
    pub(crate) fn begin_combat_damage(&mut self) -> crate::play::ActionResult<()> {
        self.combat.damage_steps += 1;
        self.combat.assigning = self.assigning();
        self.assign_combat_damage()?;        Ok(())
    }

    /// Each attacking and each blocking creature assigns combat damage equal to its power, if
    /// that is more than 0 (510.1a). An unblocked creature assigns it to what it is attacking
    /// (510.1b); a blocked creature to the creatures blocking it, as its controller divides it
    /// when there are several (510.1c); a blocking creature to the creature it blocks (510.1d).
    /// An attacking creature with trample is divided also when one creature blocks it, since
    /// what is more than lethal to that one may go to what it attacks (702.19b); blocked with
    /// no creature blocking it any more, it assigns all of it there (702.19d).
    fn assign_combat_damage(&mut self) -> crate::play::ActionResult<()> {
        self.combat.assigned.clear();
        self.combat.to_divide.clear();
        let power = |g: &Game, o: ObjId| g.power(o).filter(|p| *p > 0).map(|p| p as u32);
        let assigns = |g: &Game, o: ObjId| g.combat.assigning.contains(&o);
        for (attacker, attacked) in self.combat.attackers.clone() {
            let Some(p) = power(self, attacker).filter(|_| assigns(self, attacker)) else { continue };
            let over = self.tramples_over(attacker);
            if self.combat.blocked.contains(&attacker) {
                match (self.combat.blockers_of(attacker).as_slice(), over) {
                    ([], None) => {}
                    ([], Some(attacked)) => self.combat.assigned.push((attacker, attacked, p)),
                    ([one], None) => self.combat.assigned.push((attacker, Target::Object(*one), p)),
                    _ => self.combat.to_divide.push(attacker),
                }
            } else if self.takes(Verb::Attack, Role::To, attacked) {
                self.combat.assigned.push((attacker, attacked, p));
            }
        }
        for blocker in self.combat.blocking.clone() {
            let Some(p) = power(self, blocker).filter(|_| assigns(self, blocker)) else { continue };
            let attacking: Vec<_> = self.combat.blockers.iter().filter(|(b, _)| *b == blocker).map(|(_, a)| *a).collect();
            match attacking.as_slice() {
                [] => {},
                [one] => self.combat.assigned.push((blocker, Target::Object(*one), p)),
                _ => self.combat.to_divide.push(blocker),
            }
        }
        // Players assign their creatures' damage in APNAP order (510.1, 101.4).
        let mut pending = std::mem::take(&mut self.combat.to_divide);
        pending.sort_by_key(|o| (self.controller(*o) + self.players.len() - self.active) % self.players.len());
        self.combat.to_divide = pending;
        self.next_to_divide()?;        Ok(())
    }

    /// What an attacking creature with trample may assign damage to beyond the creatures
    /// blocking it: the player or planeswalker it is attacking (702.19b), while that can still
    /// be attacked (510.1b). Never the defending player in a planeswalker's place (702.19f).
    fn tramples_over(&self, attacker: ObjId) -> Option<Target> {
        let (_, attacked) = self.combat.attackers.iter().find(|(a, _)| *a == attacker)?;
        (self.has(attacker, "Trample") && self.takes(Verb::Attack, Role::To, *attacked)).then_some(*attacked)
    }

    /// Ask the controller to divide the next attacking or blocking creature's damage; when none is
    /// left, all of it is dealt.
    fn next_to_divide(&mut self) -> crate::play::ActionResult<()> {
        match self.combat.to_divide.first().copied() {
            Some(attacker) => {
                let mut among: Vec<Target> = if self.combat.blocking.contains(&attacker) {
                    self.combat.blockers.iter().filter(|(b, _)| *b == attacker).map(|(_, a)| Target::Object(*a)).collect()
                } else { self.combat.blockers_of(attacker).into_iter().map(Target::Object).collect() };
                among.extend(self.tramples_over(attacker));
                let amount = self.power(attacker).unwrap_or(0).max(0) as u32;
                self.decision = Some(Decision::Divide { player: self.controller(attacker), attacker, among, amount });
            }
            None => self.deal_combat_damage()?,
        }        Ok(())
    }

    /// The controller's division of an attacking or blocking creature's damage among the
    /// creatures blocking it (510.1c), and with trample what it is attacking (702.19b): all of
    /// it, and to those only. Once that player has assigned the damage of each creature they
    /// control, their total assignment is checked before anyone else assigns, and an illegal
    /// one returns the game to the moment before that player began to assign (510.1e).
    pub(crate) fn divide_damage(&mut self, attacker: ObjId, among: &[Target], amount: u32, division: Vec<(Target, u32)>) -> crate::play::ActionResult<()> {
        let distinct = division.iter().enumerate().all(|(i, (b, _))| !division[..i].iter().any(|(x, _)| x == b));
        if !distinct || !division.iter().all(|(b, _)| among.contains(b)) {
            return Err("that damage is divided only among its combat recipients (510.1c–d)".into());
        }
        if division.iter().map(|(_, n)| n).sum::<u32>() != amount {
            return Err(format!("it assigns combat damage equal to its power, {amount} (510.1a)").into());
        }
        self.decision = None;
        for (to, n) in division {
            if n > 0 {
                self.combat.assigned.push((attacker, to, n));
            }
        }
        self.combat.to_divide.retain(|a| *a != attacker);
        let player = self.controller(attacker);
        let finished = self.combat.to_divide.first().is_none_or(|next| self.controller(*next) != player);
        if finished && let Err(why) = self.assignment_legal() {
            // Only what attacking creatures assign can make a total illegal (702.19b), and
            // the active player assigns it before anyone (510.1): beginning the whole
            // assignment again is the moment before that player began.
            self.assign_combat_damage()?;
            return Err(crate::play::ActionError::Restarted(format!("{why}; the assignment begins again (510.1e)")));
        }
        self.next_to_divide()?;
        Ok(())
    }

    /// Debug builds keep the game as a player first divides combat damage in a step, to check
    /// that an illegal total assignment returns to it (510.1e).
    #[cfg(debug_assertions)]
    pub(crate) fn note_assignment_began(&mut self, dividing: bool) {
        let assigning = match self.decision { Some(Decision::Divide { player, .. }) => Some(player), _ => None };
        match (assigning, &self.assignment_began) {
            (Some(player), Some((began, _))) if player == *began => {}
            (Some(player), _) if dividing => {
                self.assignment_began = None;
                self.assignment_began = Some((player, Some(Box::new(self.clone()))));
            }
            _ => self.assignment_began = None,
        }
    }

    /// A move other than a division was taken while a player assigns: another player's
    /// concession, which they can make at any time (104.3a). No restart undoes it, so the
    /// moment before the player began is no longer one this game can return to, and the
    /// check of this assignment ends.
    #[cfg(debug_assertions)]
    pub(crate) fn note_assignment_interrupted(&mut self, dividing: bool) {
        if let Some((_, began)) = &mut self.assignment_began && !dividing {
            *began = None;
        }
    }

    /// An illegal total assignment returned the game to the moment before its player began to
    /// assign (510.1e). What is the host's since then stays the host's: the events and
    /// disclosures it was given, the departed objects nothing refers to any more, and the
    /// replay or capture of randomness scoped to this response. Dividing makes none of them.
    #[cfg(debug_assertions)]
    pub(crate) fn assert_assignment_restarted(&self, refusal: &str) {
        let (_, began) = self.assignment_began.as_ref().expect("510.1e: an assignment was begun");
        let Some(began) = began else { return };
        let mut began = (**began).clone();
        if began.log.start() < self.log.start() && self.log.start() <= began.log.end() {
            began.event_log_mut().retire_before(self.log.start());
        }
        *began.observation_waiting_mut() = self.observations.clone();
        began.captured_random = self.captured_random.clone();
        began.replay_random = self.replay_random.clone();
        began.random.recorded_only(self.random.requires_recorded());
        let mut actual = self.clone();
        began.collect_departed();
        actual.collect_departed();
        crate::state::assert_unchanged(&began, &actual, refusal);
    }

    /// Is the whole assignment of combat damage legal (510.1e)? A blocked creature assigns
    /// damage to what it is attacking only once every creature blocking it is assigned lethal
    /// damage: counting the damage already marked on each and the damage every creature is
    /// assigning to it in this step, and nothing that might change what is actually dealt
    /// (702.19b).
    fn assignment_legal(&self) -> Result<(), String> {
        let c = &self.combat;
        for (attacker, attacked) in &c.attackers {
            let over = c.assigned.iter().any(|(source, to, _)| source == attacker && to == attacked);
            let short = c.blockers_of(*attacker).into_iter().find(|blocker| {
                let to_it = || c.assigned.iter().filter(|(_, to, _)| *to == Target::Object(*blocker));
                let sources: Vec<ObjId> = to_it().map(|(source, _, _)| *source).collect();
                !self.lethal(*blocker, &sources, to_it().map(|(_, _, n)| n).sum())
            });
            if let Some(blocker) = short.filter(|_| over && c.blocked.contains(attacker)) {
                let (a, b) = (&self.obj(*attacker).chars.name, &self.obj(blocker).chars.name);
                return Err(format!("{a} assigns damage to what it attacks once {b} is assigned lethal damage (702.19b)"));
            }
        }
        Ok(())
    }

    /// All combat damage that has been assigned is dealt simultaneously (510.2): nothing is
    /// checked between one creature's damage and the next.
    fn deal_combat_damage(&mut self) -> crate::play::ActionResult<()> {
        let assigned = std::mem::take(&mut self.combat.assigned);
        self.damage_as_rule(&assigned, true)?;        Ok(())
    }

    /// As the end of combat step ends, all creatures and planeswalkers are removed from combat
    /// (511.3).
    pub(crate) fn end_combat(&mut self) {
        self.combat = Combat::default();
    }
}

/// Is this thing a creature that is attacking, blocking, blocked or unblocked (506.4, 509.1h)?
pub fn status(game: &Game, o: ObjId, status: ir::Status) -> Option<bool> {
    let c = &game.combat;
    // what is in combat is a creature (506.4: a permanent that stops being one is removed
    // from combat); asked of the layers, this would ask the layers asking it
    let creature = game.obj(o).zone == crate::state::Zone::Battlefield;
    Some(match status {
        ir::Status::Attacking => creature && c.attacking(o),
        ir::Status::Blocking => creature && c.blocking(o),
        ir::Status::Blocked => creature && c.attacking(o) && c.blocked.contains(&o),
        ir::Status::Unblocked => creature && c.attacking(o) && c.blockers_declared && !c.blocked.contains(&o),
        _ => return None,
    })
}

impl crate::retention::Trace for Combat {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        let Self { attackers, blockers, blocking, blocked, assigned, to_divide, assigning,
            declared: _, blockers_declared: _, to_block: _, damage_steps: _, first_strikers: _, declared_attackers, removed } = self;
        for (attacker, target) in attackers.iter().chain(removed) { attacker.trace(visit); target.trace(visit); }
        for (blocker, attacker) in blockers { blocker.trace(visit); attacker.trace(visit); }
        blocking.trace(visit);
        blocked.trace(visit);
        for (source, target, _) in assigned { source.trace(visit); target.trace(visit); }
        to_divide.trace(visit);
        assigning.trace(visit);
        declared_attackers.trace(visit);
        // first_strikers is only an identity-membership query when the next damage
        // step begins; a creature that left combat contributes no payload root.
    }
}
