//! Resolved, turn-long prevention: fixed next-N shields (615.7) and prevent-all (615.1).
use crate::event::Target;
use crate::operation::ReplacementId;
use crate::retention::Trace;
use crate::select::Ctx;
use crate::statement::Affected;
use crate::{Game, ObjId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Effect {
    pub id: ReplacementId,
    pub ctx: Ctx<'static>,
    pub to: Option<Affected>,
    pub by: Option<Affected>,
    pub to_or_by: bool,
    pub kind: ir::event::DamageKind,
    /// None prevents all; Some is the shield's unconsumed amount.
    pub remaining: Option<u32>,
}
impl Effect {
    pub fn matches(&self, game: &Game, source: ObjId, to: Target, combat: bool) -> bool {
        if matches!((self.kind, combat),
            (ir::event::DamageKind::Combat, false) | (ir::event::DamageKind::Noncombat, true)) {
            return false;
        }
        let recipient = self.to.as_ref().is_none_or(|a| crate::statement::affects(game, &self.ctx, a, to));
        let source_matches = self.by.as_ref().is_none_or(|a| {
            // Source predicates read the departed battlefield incarnation (608.2h).
            let mut ctx = self.ctx.clone();
            if game.obj(source).zone == crate::Zone::Gone && game.obj(source).left == Some(crate::Zone::Battlefield) {
                ctx.was = Some(source);
            }
            crate::statement::affects(game, &ctx, a, source.into())
        });
        if self.to_or_by { recipient || source_matches } else { recipient && source_matches }
    }
}
impl Trace for Effect {
    fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        self.id.source.trace(visit);
        self.ctx.trace(visit);
        fn affected(a: &Affected, visit: &mut dyn FnMut(ObjId)) {
            match a {
                Affected::Things(things) => things.trace(visit),
                Affected::Union(items) => {
                    for item in items {
                        affected(item, visit);
                    }
                }
                Affected::Compiled { .. } => a.trace(visit),
            }
        }
        for a in self.to.iter().chain(&self.by) {
            affected(a, visit);
        }
    }
}
impl Game {
    /// Install one already-resolved, fixed-recipient next-N shield without a cast,
    /// target event, payment or trigger (615.7). This bounded premise authenticates
    /// a single pure printed instruction; it is not arbitrary continuation restore.
    pub fn set_initial_prevention(&mut self, source:ObjId, paragraph:usize,
        controller:usize, recipient:Target, remaining:u32) -> Result<ir::event::DamageKind,String> {
        use crate::binding::{AbilityKey,Origin};
        use crate::program::{Instr,action::{Action,Operand}};
        if controller>=self.players.len() || !self.battlefield.contains(&source)
            || self.obj(source).copy_acquisition.is_some() || !matches!(self.copy_origin(source),Origin::Own) {
            return Err("initial prevention requires an unchanged live printed source".into());
        }
        if controller!=self.controller(source) {
            return Err("initial prevention requires current-controller producer history".into());
        }
        let index=paragraph.checked_sub(1).ok_or("invalid prevention producer paragraph")?;
        let def=self.def(source).ok_or("prevention source definition unavailable")?;
        let Some(ir::Ability::Activated{cost,effect})=def.abilities.get(index) else {
            return Err("initial prevention requires a printed activated producer".into());
        };
        if !matches!(effect.0.as_slice(),[ir::Sentence::Do(_)])
            || cost.0.iter().any(|part|!matches!(part,ir::CostPart::Mana(_)|ir::CostPart::Tap|ir::CostPart::Untap)) {
            return Err("initial prevention producer needs unsupported activation/cost history".into());
        }
        let program=crate::program::definition_program(def,index).ok_or("prevention producer unavailable")?
            .map_err(|e|e.0)?;
        let [Instr::Capture{operand:Operand::Targets(0),out}, Instr::Action {action:Action::Prevent{to,by,to_or_by:false,kind,next:Some(capacity)},..}]=program.instrs.as_slice()
            else {return Err("initial prevention requires one pure fixed next-N instruction".into())};
        if to.as_slice()!=[Operand::Reference(*out)] || !by.is_empty() || remaining==0 || remaining>*capacity {
            return Err("invalid initial prevention recipient or remaining capacity".into());
        }
        let kind=*kind;
        let mut ctx=Ctx::new(controller,source);ctx.ability=Some(AbilityKey::printed(index));
        // This construction profile additionally requires a currently legal
        // announcement target. Existing resolved effects do not recheck targeting;
        // projection remains valid after a recipient later gains shroud, etc.
        self.legal_targets(&ctx,&program.targets,&[vec![recipient]])?;
        // A private cardless occurrence owns resource identity only. It is never
        // relabeled as the source permanent or exported as a historical spell.
        let occurrence=self.new_object(None,self.obj(source).chars.clone(),controller,controller,crate::Zone::Gone);
        self.obj_mut(occurrence).ability=Some((Some(source),index));
        self.begin_prevention(Effect{id:ReplacementId::from_ability(occurrence,AbilityKey::printed(index),1),
            ctx,to:Some(Affected::Things(vec![recipient])),by:None,to_or_by:false,kind,remaining:Some(remaining)});
        Ok(kind)
    }

    /// Display the retained finite capacity; staged usage remains private to its operation.
    pub fn prevention_capacity(&self, id: ReplacementId) -> Option<u32> {
        self.prevention.iter().find(|e| e.id == id).and_then(|e| e.remaining)
    }
    pub(crate) fn begin_prevention(&mut self, effect: Effect) {
        if effect.remaining == Some(0) || matches!(&effect.to, Some(Affected::Things(things)) if things.is_empty()) {
            return;
        }
        self.prevention.push(effect);
    }
    pub(crate) fn commit_prevention(&mut self, used: &[(ReplacementId, u32)]) {
        for &(ref id, amount) in used {
            let effect = self.prevention.iter_mut().find(|e| e.id == *id).expect("retained shield");
            let remaining = effect.remaining.as_mut().expect("finite shield");
            *remaining = remaining.checked_sub(amount).expect("staged shield capacity");
        }
        self.prevention.retain(|e| e.remaining != Some(0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Answer, Choice, Decision};
    use crate::{CardDef, Event, Move, Zone};
    fn pinned(name: &str) -> CardDef {
        let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        let def = CardDef::from_faces(CardDef::catalog(&faces).get(name).unwrap()).unwrap();
        crate::play::check(&def).unwrap();
        def
    }
    fn game() -> Game {
        Game::new(
            vec![
                pinned("Mending Hands"),
                pinned("Twinflame Tyrant"),
                CardDef::new("Red", "{R}", "Creature", "", Some((2, 20))).unwrap(),
                CardDef::new("Blue", "{U}", "Creature", "", Some((2, 20))).unwrap(),
                pinned("Paladin en-Vec"),
                pinned("Sunspine Lynx"),
                pinned("Furnace of Rath"),
            ],
            2,
            20,
        )
    }
    // Explicit fixed-shield fixtures isolate the proposal/resource contract. Full spell
    // creation, modal target announcement and activation are asserted separately below.
    fn shield(g: &mut Game, on: Target, n: u32) -> ReplacementId {
        let occurrence = g.put(0, 0, Zone::Stack);
        let id = ReplacementId::from_ability(occurrence, crate::binding::AbilityKey::printed(0), 0);
        g.begin_prevention(Effect {
            id: id.clone(),
            ctx: Ctx::new(0, occurrence),
            to: Some(Affected::Things(vec![on])),
            by: None,
            to_or_by: false,
            kind: ir::event::DamageKind::Any,
            remaining: Some(n),
        });
        id
    }
    fn order(g: &mut Game, pick: impl Fn(DamageOption) -> bool) {
        let Some(Decision::Choose { player, choice: Choice::DamageOrder { among, .. } }) = g.decision.clone() else {
            panic!("{:?}", g.decision)
        };
        let o = among.iter().find(|o| pick((*o).clone())).unwrap().clone();
        g.act(player, Move::Choose(Answer::DamageOrder(o))).unwrap();
    }
    use crate::operation::DamageOption;
    #[test]
    fn r615_7_full_pinned_forms_and_unsupported_boundaries() {
        for name in ["Mending Hands", "Healing Salve", "Fog", "Holy Day", "Kami of False Hope"] {
            pinned(name);
        }
        let mana=CardDef::new("Mana Ward","{0}","Artifact","{T}: Add {R}. Prevent all combat damage that would be dealt this turn.",None).unwrap();
        assert_eq!(crate::play::check(&mana).unwrap_err().0, "prevention combined with mana in an ability needs an explicit nonstack occurrence identity");
        for (text, reason) in [
            ("Prevent the next X damage that would be dealt to any target this turn.", "prevention: fixed next-N or all"),
            ("Prevent all damage that would be dealt this combat.", "prevention: only this-turn duration"),
            ("Prevent the next 3 damage that would be dealt to any target.", "prevention: only this-turn duration"),
            ("Prevent the next 3 damage that would be dealt by target creature this turn.", "prevention shield: one target, no source restriction"),
        ] {
            let def = CardDef::new("Unsupported", "{0}", "Instant", text, None).unwrap();
            assert_eq!(crate::play::check(&def).unwrap_err().0, reason, "{text}");
        }
    }
    #[test]
    fn r615_1_full_fog_holy_day_and_kami_prevent_the_simultaneous_combat_batch() {
        for name in ["Fog", "Holy Day", "Kami of False Hope"] {
            let mut g = Game::new(
                vec![pinned(name), CardDef::new("Dealer", "{R}", "Creature", "", Some((2, 20))).unwrap()],
                2,
                20,
            );
            g.players[0].pool.colors = [10; 5];
            if name == "Kami of False Hope" {
                let source = g.put(0, 0, Zone::Battlefield);
                g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
            } else {
                let spell = g.put(0, 0, Zone::Hand);
                let mut selected = crate::state::Pool::default();
                selected.colors[if name == "Fog" { 4 } else { 0 }] = 1;
                g.act(0, Move::Paying(selected, Box::new(Move::Cast { card: spell, targets: vec![] }))).unwrap();
            }
            for _ in 0..2 {
                g.act(g.priority, Move::Pass).unwrap();
            }
            let source = g.put(0, 1, Zone::Battlefield);
            let start = g.log.len();
            g.damage_as_rule(&[(source, Target::Player(1), 2), (source, Target::Player(0), 2)], true).unwrap();
            assert_eq!((g.players[0].life, g.players[1].life), (20, 20));
            assert_eq!(g.log.len(), start);
            assert!(g.decision.is_none());
            g.damage(source, Target::Player(1), 2);
            assert_eq!(g.players[1].life, 18);
        }
    }
    #[test]
    fn r615_7_allocation_is_mandatory_nongreedy_validated_and_cloned_without_consumption() {
        let mut g = game();
        let red = g.put(0, 2, Zone::Battlefield);
        let blue = g.put(0, 3, Zone::Battlefield);
        let id = shield(&mut g, Target::Player(1), 3);
        let start = g.log.len();
        g.damage_as_rule(
            &[(red, Target::Player(1), 2), (blue, Target::Player(1), 2), (red, Target::Player(1), 2)],
            true,
        ).unwrap();
        let Some(Decision::Choose { player: 1, choice: Choice::Prevention { total: 3, hits, .. } }) = &g.decision else {
            panic!("expected source allocation: {:?}", g.decision);
        };
        assert_eq!(hits, &vec![
            crate::damage::ProposedHit { id: 0, source: red, amount: 4 },
            crate::damage::ProposedHit { id: 1, source: blue, amount: 2 },
        ]);
        let before = format!("{g:?}");
        // Sum, duplicate identity, per-source capacity, missing source, stale identity.
        for allocation in [
            vec![(0, 1), (1, 1)],
            vec![(0, 1), (0, 2)],
            vec![(0, 0), (1, 3)],
            vec![(0, 3)],
            vec![(0, 1), (99, 2)],
        ] {
            assert_eq!(g.act(1, Move::Choose(Answer::Prevention(allocation))),
                Err("allocate the required shield total over each offered hit within its capacity (615.7)".into()));
            assert_eq!(format!("{g:?}"), before);
        }
        let allocation = vec![(1, 1), (0, 2)];
        assert_eq!(g.act(0, Move::Choose(Answer::Prevention(allocation.clone()))),
            Err("that decision is another player's".into()));
        assert_eq!(format!("{g:?}"), before);
        assert_eq!(g.collect_departed(), 0);
        let mut copy = g.clone();
        for branch in [&mut g, &mut copy] {
            branch.act(1, Move::Choose(Answer::Prevention(allocation.clone()))).unwrap();
            assert!(branch.prevention.iter().all(|e| e.id != id));
            assert_eq!(branch.players[1].life, 17);
            let amounts: Vec<_> = branch
                .log
                .iter()
                .skip(start)
                .filter_map(|e| if let Event::DamageDealt { amount, .. } = e { Some(*amount) } else { None })
                .collect();
            assert_eq!(amounts, [2, 1]);
        }
    }
    /// CR615.7: one source needs no allocation, even if the instruction was fragmented.
    #[test]
    fn r615_7_same_source_shield_partial_exact_and_excess_capacity() {
        for capacity in [2, 6, 9] {
            let mut g = game();
            let source = g.put(0, 2, Zone::Battlefield);
            let id = shield(&mut g, Target::Player(1), capacity);
            g.damage_as_rule(&[(source, Target::Player(1), 4), (source, Target::Player(1), 2)], false).unwrap();
            assert!(g.decision.is_none());
            assert_eq!(g.players[1].life, 20 - 6u32.saturating_sub(capacity) as i32);
            assert_eq!(g.prevention_capacity(id.clone()), (capacity > 6).then_some(capacity - capacity.min(6)));
            g.damage_as_rule(&[(source, Target::Player(1), 4)], false).unwrap();
            assert!(g.decision.is_none());
            assert_eq!(g.players[1].life, 20 - 6u32.saturating_sub(capacity) as i32 - 4u32.saturating_sub(capacity.saturating_sub(6)) as i32);
            assert!(g.prevention.iter().all(|effect| effect.id != id));
        }
    }
    #[test]
    fn r616_1_shared_shield_competes_with_one_hits_single_modifier() {
        for outcome in 0..3 {
            let mut g = game();
            g.put(0, 1, Zone::Battlefield);
            let a = g.put(1, 3, Zone::Battlefield);
            let b = g.put(0, 2, Zone::Battlefield);
            shield(&mut g, Target::Player(1), 2);
            let start = g.log.len();
            g.damage_as_rule(&[(a, Target::Player(1), 1), (b, Target::Player(1), 1)], true).unwrap();
            assert!(
                matches!(g.decision,Some(Decision::Choose{choice:Choice::DamageOrder{ref among,..},..}) if among.len()==2)
            );
            if outcome == 0 {
                order(&mut g, |o| matches!(o, DamageOption::Shield(_)));
            } else {
                order(&mut g, |o| matches!(o, DamageOption::Modifier(_)));
                assert_eq!(g.players[1].life, 20);
                assert_eq!(g.prevention[0].remaining, Some(2));
                let allocation = if outcome == 1 { vec![(0, 0), (1, 2)] } else { vec![(0, 1), (1, 1)] };
                g.act(1, Move::Choose(Answer::Prevention(allocation))).unwrap();
            }
            let hits: Vec<_> = g
                .log
                .iter()
                .skip(start)
                .filter_map(|e| {
                    if let Event::DamageDealt { source, amount, .. } = e { Some((*source, *amount)) } else { None }
                })
                .collect();
            assert_eq!(
                hits,
                match outcome {
                    0 => vec![],
                    1 => vec![(a, 1)],
                    _ => vec![(b, 1)],
                }
            );
            assert!(g.prevention.is_empty());
        }
    }
    #[test]
    fn r702_16e_protection_order_can_preserve_or_spend_a_shield() {
        for protection_first in [false, true] {
            let mut g = game();
            let red = g.put(0, 2, Zone::Battlefield);
            let paladin = g.put(1, 4, Zone::Battlefield);
            shield(&mut g, paladin.into(), 3);
            g.damage(red, paladin.into(), 1);
            let Some(Decision::Choose { player: 1, choice: Choice::DamageOrder { ref among, .. } }) = g.decision else {
                panic!("{:?}", g.decision)
            };
            assert_eq!(among.len(), 2, "one structural protection even with multiple qualities");
            order(&mut g, |o| {
                if protection_first {
                    matches!(o, DamageOption::Protection { .. })
                } else {
                    matches!(o, DamageOption::Shield(_))
                }
            });
            assert!(g.decision.is_none());
            assert_eq!(g.obj(paladin).damage, 0);
            assert_eq!(g.prevention[0].remaining, Some(if protection_first { 3 } else { 2 }));
        }
    }
    #[test]
    fn r616_1_two_shields_and_modifiers_recheck_remaining_hits_without_early_commit() {
        for modifiers_first in [false, true] {
            let mut g = game();
            g.put(0, 6, Zone::Battlefield);
            let a = g.put(0, 2, Zone::Battlefield);
            let b = g.put(0, 3, Zone::Battlefield);
            let big = shield(&mut g, Target::Player(1), 3);
            shield(&mut g, Target::Player(1), 1);
            g.damage_as_rule(&[(a, Target::Player(1), 2), (b, Target::Player(1), 2)], false).unwrap();
            if modifiers_first {
                order(&mut g, |o| matches!(o,DamageOption::Modifier(o) if o.hit==0));
                order(&mut g, |o| matches!(o,DamageOption::Modifier(o) if o.hit==1));
            }
            order(&mut g, |o| o == DamageOption::Shield(big.clone()));
            g.act(1, Move::Choose(Answer::Prevention(vec![(0, 1), (1, 2)]))).unwrap();
            assert_eq!(g.players[1].life, 20);
            assert_eq!(g.prevention[0].remaining, Some(3));
            if modifiers_first {
                g.act(1, Move::Choose(Answer::Prevention(vec![(0, 1), (1, 0)]))).unwrap();
            } else {
                order(&mut g, |o| matches!(o, DamageOption::Modifier(_)));
            }
            assert!(g.decision.is_none());
            assert!(g.prevention.is_empty());
            assert_eq!(g.players[1].life, if modifiers_first { 16 } else { 19 });
        }
    }
    #[test]
    fn r615_12_unpreventable_damage_leaves_resources_and_still_modifies() {
        let mut g = game();
        let red = g.put(0, 2, Zone::Battlefield);
        let paladin = g.put(1, 4, Zone::Battlefield);
        g.put(0, 5, Zone::Battlefield);
        g.put(0, 6, Zone::Battlefield);
        shield(&mut g, paladin.into(), 4);
        g.damage(red, paladin.into(), 1);
        assert!(g.decision.is_none());
        assert_eq!(g.obj(paladin).damage, 2);
        assert_eq!(g.prevention[0].remaining, Some(4));
    }
    #[test]
    fn r615_7_partial_use_in_separate_batches_and_no_inheritance_after_departure() {
        let mut g = game();
        let red = g.put(0, 2, Zone::Battlefield);
        let blue = g.put(1, 3, Zone::Battlefield);
        shield(&mut g, blue.into(), 3);
        g.damage(red, blue.into(), 2);
        assert_eq!(g.obj(blue).damage, 0);
        assert_eq!(g.prevention[0].remaining, Some(1));
        g.damage(red, blue.into(), 2);
        assert_eq!(g.obj(blue).damage, 1);
        assert!(g.prevention.is_empty());
        shield(&mut g, blue.into(), 4);
        g.destroy(blue);
        let next = g.put(1, 3, Zone::Battlefield);
        g.damage(red, next.into(), 2);
        assert_eq!(g.obj(next).damage, 2);
    }
}

#[cfg(test)]
mod lifetime_tests {
    use super::*;
    use crate::{CardDef, Zone};
    use ir::Part;
    #[test]
    fn r514_2_cleanup_expires_prevention_separately_from_regeneration() {
        let def = CardDef::new("Source", "{0}", "Creature", "", Some((2, 20))).unwrap();
        let mut g = Game::new(vec![def], 2, 20);
        let source = g.put(0, 0, Zone::Battlefield);
        g.begin_prevention(Effect {
            id: ReplacementId::from_ability(source, crate::binding::AbilityKey::printed(0), 0),
            ctx: Ctx::new(0, source),
            to: Some(Affected::Things(vec![Target::Player(1)])),
            by: None,
            to_or_by: false,
            kind: ir::event::DamageKind::Any,
            remaining: Some(3),
        });
        g.begin_part(Part::Cleanup).unwrap();
        assert!(g.prevention.is_empty());
    }
}

#[cfg(test)]
mod continuation_tests {
    use super::*;
    use crate::state::{Answer, Choice, Decision};
    use crate::{CardDef, Event, Move, Zone};
    #[test]
    fn r120_4_shield_consumption_stays_staged_through_infect_result_replacements_and_clone() {
        let source = CardDef::new("Dealer", "{R}", "Creature", "Infect\nToxic 2\nLifelink", Some((4, 20))).unwrap();
        let mut plus=CardDef::new("Plus","{0}","Enchantment","If one or more energy counters would be put on a player, that many plus one energy counters are put on that player instead.",None).unwrap();
        let mut double=CardDef::new("Double","{0}","Enchantment","If one or more energy counters would be put on a player, twice that many energy counters are put on that player instead.",None).unwrap();
        // Typed poison-replacement fixtures, not text-admission claims.
        for d in [&mut plus, &mut double] {
            let crate::replace::Replacement::Counters { kinds: crate::replace::CounterScope::Named(kind), .. } = d.replacements[0].result.as_mut().unwrap() else {
                panic!("fixture")
            };
            *kind = "poison".into();
        }
        let mut g = Game::new(vec![source, plus, double], 2, 20);
        let source = g.put(0, 0, Zone::Battlefield);
        let plus = g.put(0, 1, Zone::Battlefield);
        g.put(0, 2, Zone::Battlefield);
        let id = ReplacementId::from_ability(source, crate::binding::AbilityKey::printed(0), 0);
        g.begin_prevention(Effect {
            id,
            ctx: Ctx::new(0, source),
            to: Some(Affected::Things(vec![Target::Player(1)])),
            by: None,
            to_or_by: false,
            kind: ir::event::DamageKind::Any,
            remaining: Some(3),
        });
        let start = g.log.len();
        g.damage_as_rule(&[(source, Target::Player(1), 4)], true).unwrap();
        assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::Replacement { count: 3, .. }, .. })));
        assert_eq!(g.prevention[0].remaining, Some(3));
        assert!(g.counters(Target::Player(1)).is_empty());
        assert_eq!(g.players[0].life, 20);
        assert_eq!(g.log.len(), start);
        let mut copy = g.clone();
        for (branch, plus_first, poison) in [(&mut g, true, 8), (&mut copy, false, 7)] {
            let Some(Decision::Choose { player, choice: Choice::Replacement { among, .. } }) = branch.decision.clone()
            else {
                panic!("result question")
            };
            let id = among.iter().find(|e| (e.source == plus) == plus_first).unwrap().clone();
            branch.act(player, Move::Choose(Answer::Replacement(id))).unwrap();
            assert!(branch.prevention.is_empty());
            assert_eq!(branch.counters(Target::Player(1))["poison"], poison);
            assert_eq!(branch.players[0].life, 21);
            assert_eq!(branch.players[1].life, 20);
            assert_eq!(
                branch.log.iter().skip(start).filter(|e| matches!(e, Event::DamageDealt { amount: 1, .. })).count(),
                1
            );
        }
    }
    #[test]
    fn r615_7_repeated_activations_have_distinct_stack_occurrences_and_preserve_semantic_source() {
        let well = CardDef::new(
            "Well",
            "{0}",
            "Artifact",
            "{0}: Prevent the next 3 damage that would be dealt to any target this turn.",
            None,
        )
        .unwrap();
        crate::play::check(&well).unwrap();
        let mut g = Game::new(vec![well], 2, 20);
        let well = g.put(0, 0, Zone::Battlefield);
        for _ in 0..2 {
            g.act(
                0,
                Move::Activate { source: well, ability: 0, targets: vec![vec![Target::Player(1)]], paying: vec![] },
            )
            .unwrap();
            for _ in 0..2 {
                g.act(g.priority, Move::Pass).unwrap();
            }
        }
        assert_eq!(g.prevention.len(), 2);
        assert_ne!(g.prevention[0].id.source, g.prevention[1].id.source);
        assert!(g.prevention.iter().all(|e| e.ctx.source == Some(well) && e.id.source != well));
        let ids: Vec<_> = g.prevention.iter().map(|e| e.id.source).collect();
        g.collect_departed();
        assert!(ids.iter().all(|id| g.obj(*id).zone == Zone::Gone));
    }
}

#[cfg(test)]
mod modal_tests {
    use super::*;
    use crate::{CardDef, Move, Zone};
    #[test]
    fn r700_2_two_selected_prevention_modes_get_distinct_instruction_parts() {
        let def=CardDef::new("Two Wards","{0}","Instant","Choose two —\n• Prevent the next 3 damage that would be dealt to any target this turn.\n• Prevent the next 4 damage that would be dealt to any target this turn.",None).unwrap();
        crate::play::check(&def).unwrap();
        let mut g = Game::new(vec![def], 2, 20);
        let spell = g.put(0, 0, Zone::Hand);
        g.act(
            0,
            Move::Modes(
                vec![0, 1],
                Box::new(Move::Cast { card: spell, targets: vec![vec![Target::Player(1)], vec![Target::Player(1)]] }),
            ),
        )
        .unwrap();
        for _ in 0..2 {
            g.act(g.priority, Move::Pass).unwrap();
        }
        assert_eq!(g.prevention.len(), 2);
        assert_eq!(g.prevention[0].id.source, g.prevention[1].id.source);
        assert_eq!(g.prevention[0].id.ability(), g.prevention[1].id.ability());
        assert_ne!(g.prevention[0].id.part(), g.prevention[1].id.part());
        assert_eq!(g.prevention[0].remaining, Some(3));
        assert_eq!(g.prevention[1].remaining, Some(4));
    }
}
