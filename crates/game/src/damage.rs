//! Damage proposals and frozen results (120.4). Counter replacement ordering is retained
//! by Operation::Damage; no result is written until every placement is ready.
use crate::operation::{Placement, Prepared, ReplacementId};
use crate::{Event, Game, ObjId, PlayerId, Target};
use ir::{CardType, Role, Verb};

/// Actual damage from one completed occurrence, independent of later zone or
/// characteristic changes (120.10, 608.2i). The threshold is frozen before results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedDamage {
    pub(crate) recipient: Target,
    pub(crate) sources: Vec<ObjId>,
    pub(crate) amount: u32,
    pub(crate) excess: u32,
    pub(crate) combat: bool,
}

fn completed_damage(game: &Game, hits: &[Hit], combat: bool) -> crate::Res<Vec<CompletedDamage>> {
    // Any later unqualified completed-damage quantity may sum all recipients.
    // Establish that bound before committing this occurrence, not in a later follower.
    hits.iter().try_fold(0i64, |sum, hit| sum.checked_add(i64::from(hit.amount))
        .ok_or_else(|| crate::Unsupported("completed damage total exceeds the engine numerical range".into())))?;
    let mut result = Vec::new();
    for hit in hits {
        if result.iter().any(|r: &CompletedDamage| r.recipient == hit.to) { continue; }
        let same: Vec<_> = hits.iter().filter(|h| h.to == hit.to).collect();
        let amount = same.iter().try_fold(0u32, |n, h| n.checked_add(h.amount)
            .ok_or_else(|| crate::Unsupported("completed damage exceeds the engine numerical range".into())))?;
        let mut excess = 0;
        if let Target::Object(on) = hit.to {
            let seen = game.seen(on, None);
            let object = game.obj(on);
            if seen.is(CardType::Creature) {
                let remaining = (i64::from(seen.pt.map_or(0, |(_, t)| t)) - i64::from(object.damage)).max(0);
                // Only an actual positive deathtouch hit can supply this shortcut.
                let lethal = if object.deathtouched { 0 } else if same.iter().any(|h| h.deathtouch) { remaining.min(1) } else { remaining };
                excess = u32::try_from((i64::from(amount) - lethal).max(0)).expect("difference is bounded by actual damage");
            }
            for (kind, counter) in [(CardType::Planeswalker, "loyalty"), (CardType::Battle, "defense")] {
                if seen.is(kind) {
                    let before = game.counters(on).get(counter).copied().unwrap_or(0);
                    excess = excess.max(amount.saturating_sub(before));
                }
            }
        }
        let mut sources = Vec::new();
        for h in same { if !sources.contains(&h.source) { sources.push(h.source); } }
        result.push(CompletedDamage { recipient: hit.to, sources, amount, excess, combat });
    }
    Ok(result)
}

/// Operation-local identity of one normalized source-recipient damage event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HitId(usize);

#[derive(Debug, Clone)]
struct Hit {
    id: HitId,
    source: ObjId,
    to: Target,
    amount: u32,
    deathtouch: bool,
    mark: bool,
    planeswalker: bool,
    lose_life: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Outcome {
    combat: bool,
    hits: Vec<Hit>,
    gains: Vec<(PlayerId, u32)>,
    completed: Vec<CompletedDamage>,
}

impl Outcome {
    /// Unit-fixture route: eligibility/protection once, then freeze result facts.
    /// Production callers also finish proposal modifiers before this freeze.
    #[cfg(test)]
    pub(crate) fn propose(game: &Game, damage: &[(ObjId, Target, u32)], combat: bool) -> (Self, Vec<Placement>) {
        let mut proposals = Proposals::new(game, damage, combat).unwrap();
        assert!(proposals.advance(game).unwrap().is_none(), "fixture has a damage question");
        let (outcome, placements, _) = proposals.freeze(game).unwrap();
        (outcome, placements)
    }
    fn propose_hits(game: &Game, damage: &[(usize,ObjId,Target,u32)], combat: bool) -> crate::Res<(Self,Vec<Placement>)> {
        let mut hits = Vec::new();
        let mut gains: Vec<(ObjId, PlayerId, u32)> = Vec::new();
        let mut placements: Vec<Placement> = Vec::new();
        for &(id, source, to, amount) in damage {
            if amount == 0 || !game.takes(Verb::DealDamage, Role::To, to) { continue; }
            let controller = game.controller(source);
            let seen = game.seen_for(source, crate::layers::Aspect::Abilities);
            let has = |name| seen.has(ir::Keyword::from_name(name).unwrap());
            if has("Lifelink") {
                if let Some((_, _, total)) = gains.iter_mut().find(|(s, _, _)| *s == source) {
                    *total = total.checked_add(amount).ok_or_else(|| crate::Unsupported(
                        "life gain amount exceeds the engine numerical range".into()))?;
                } else { gains.push((source, controller, amount)); }
            }
            let (planeswalker, creature) = match to {
                Target::Player(_) => (false, false),
                Target::Object(object) => {
                    let recipient = game.seen_for(object, crate::layers::Aspect::Types);
                    (recipient.is(CardType::Planeswalker), recipient.is(CardType::Creature))
                }
            };
            let infect = has("Infect");
            let counter_damage = creature && (has("Wither") || infect);
            let toxic = if combat && seen.is(CardType::Creature) && matches!(to, Target::Player(_)) {
                seen.total_toxic_value()
            } else { 0 };
            let counter = if counter_damage { Some(("-1/-1", amount)) }
                else if matches!(to, Target::Player(_)) {
                    let poison = crate::operation::counter_add(if infect { amount } else { 0 }, toxic)?;
                    (poison > 0).then_some(("poison", poison))
                } else { None };
            if let Some((kind, count)) = counter {
                // One controller's simultaneous results are one placement (120.3b/d/g).
                // Kind follows the recipient; different controllers keep separate histories.
                if let Some(placed) = placements.iter_mut().find(|p| p.on == to && p.by == controller) {
                    placed.count = crate::operation::counter_add(placed.count, count)?;
                } else {
                    placements.push(Placement { on: to, kind: kind.into(), count, by: controller });
                }
            }
            hits.push(Hit { id: HitId(id), source, to, amount, deathtouch: creature && has("Deathtouch"), mark: creature && !counter_damage, planeswalker, lose_life: !infect });
        }
        let gains = gains.into_iter().map(|(_, player, amount)| Ok((player, game.life_gain_amount(player, amount)?)))
            .collect::<crate::Res<_>>()?;
        let completed = completed_damage(game, &hits, combat)?;
        Ok((Self { combat, hits, gains, completed }, placements))
    }

    /// A consuming, non-suspending commitment. Damage triggers read the pre-result state
    /// (120.4b); result events are captured at the next ordinary trigger check (120.4c).
    pub(crate) fn commit(self, game: &mut Game, counters: Prepared) -> crate::Res<Vec<CompletedDamage>> {
        counters.check(game)?;
        // the damage dealt at once is one event (510.2, 603.2c): "one or more creatures deal
        // combat damage to a player" triggers once for it
        let from = game.event_log_mut().end();
        for (at, hit) in self.hits.iter().enumerate() {
            debug_assert!(self.hits[..at].iter().all(|other| other.id != hit.id));
            let dealt = Event::DamageDealt { source: hit.source, to: hit.to, amount: hit.amount, combat: self.combat };
            // "dealt damage by ~ this turn" (120.1)
            game.note(&dealt);
            game.event_log_mut().push(dealt);
        }
        for result in self.completed.iter().filter(|r| r.excess > 0) {
            game.event_log_mut().push(Event::ExcessDamageDealt { to: result.recipient, amount: result.excess, combat: self.combat });
        }
        game.finish_simultaneous(from, crate::layers::Working::before(Vec::new()));
        // None of the damage callers runs inside an unfinished finish_simultaneous span.
        // This only captures: returns and other procedure advancement wait for completion.
        if !self.hits.is_empty() { game.capture_triggers(); }
        for hit in self.hits {
            if self.combat && let Target::Player(player) = hit.to { game.commander_damage(hit.source, player, hit.amount); }
            match hit.to {
                // A life total out of range refuses the whole action, which is restored
                // (Game::act); no result is applied in part (107.1b).
                Target::Player(player) => { if hit.lose_life { game.lose_life(player, hit.amount)?; } },
                Target::Object(object) => {
                    if hit.planeswalker { game.remove_counters(object, "loyalty", hit.amount); }
                    let obj = game.obj_mut(object);
                    if hit.mark { obj.damage += hit.amount as i32; }
                    // Counter damage is still damage: deathtouch is independent of marking it.
                    obj.deathtouched |= hit.deathtouch;
                }
            }
        }
        let installed = counters.install(game); // preflight above precedes every mutation
        for (player, amount) in self.gains { game.gain_life_result(player, amount)?; }
        // CR120.4c/d, 603.10: recipient characteristics include every damage result,
        // including frozen lifelink applicability. No trigger check splits this event.
        installed.record(game);
        Ok(self.completed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Answer, Choice, Decision};
    use crate::{CardDef, Move, Zone};

    fn creature(name: &str, text: &str, pt: (i32, i32)) -> CardDef {
        CardDef::new(name, "{0}", "Creature", text, Some(pt)).unwrap()
    }
    fn game() -> Game {
        let plus = "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.";
        let times = "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on it instead.";
        let mut defs = vec![
            creature("Source", "Wither\nLifelink\n{0}: This creature deals 2 damage to target creature.", (2, 8)),
            creature("Wall", "", (0, 20)),
            CardDef::new("Plus", "{0}", "Enchantment", plus, None).unwrap(),
            CardDef::new("Times", "{0}", "Enchantment", times, None).unwrap(),
            CardDef::new("Fight", "{0}", "Instant", "Target creature you control fights target creature you don't control.", None).unwrap(),
            CardDef::new("Boon", "{0}", "Enchantment", "If you would gain life, you gain twice that much life instead.", None).unwrap(),
            CardDef::new("Prison", "{0}", "Enchantment", "", None).unwrap(),
            creature("Watcher", "Whenever a creature is dealt damage, you gain 1 life.", (1, 1)),
            CardDef::new("Guard", "{0}", "Enchantment", "As long as you control a creature with power 2 or greater, you can't gain life.", None).unwrap(),
        ];
        // Typed runtime fixtures, NOT an admission claim for minus-counter replacement text.
        // Production lowering deliberately still admits only the existing plus-counter family.
        for def in &defs { crate::play::check(def).unwrap(); }
        for def in &mut defs[2..4] {
            let crate::replace::Replacement::Counters { kinds: crate::replace::CounterScope::Named(kind), .. } = def.replacements[0].result.as_mut().unwrap() else { panic!("fixture") };
            *kind = "-1/-1".into();
        }
        Game::new(defs, 3, 20)
    }
    fn setup(g: &mut Game) -> (ObjId, ObjId) {
        let source = g.put(0, 0, Zone::Battlefield);
        let target = g.put(1, 1, Zone::Battlefield);
        g.put(1, 2, Zone::Battlefield);
        g.put(1, 3, Zone::Battlefield);
        (source, target)
    }
    fn answer(g: &mut Game, source_of_first: usize) {
        let Some(Decision::Choose { player, choice: Choice::Replacement { on, among, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
        assert_eq!(player, 1, "affected creature's controller, not counter placer");
        assert!(matches!(on, Target::Object(_)));
        let effect = among.iter().find(|id| g.obj(id.source).card == Some(source_of_first)).unwrap().clone();
        g.act(player, Move::Choose(Answer::Replacement(effect))).unwrap();
    }
    fn unchanged(g: &Game, source: ObjId, target: ObjId, from: usize) {
        assert_eq!(g.players[0].life, 20);
        assert_eq!(g.obj(target).damage, 0);
        assert!(g.counters(target).is_empty());
        assert_eq!(g.obj(source).damage, 0);
        assert!(!g.log.iter().skip(from).any(|e| matches!(e, Event::DamageDealt { .. } | Event::CountersPut { .. } | Event::LifeGained { .. })));
    }
    #[test]
    fn r120_4_result_choice_freezes_the_batch_and_clone_commits_once() {
        let mut g = game();
        let (source, target) = setup(&mut g);
        let from = g.log.iter().count();
        g.damage(source, target.into(), 2);
        unchanged(&g, source, target, from);
        assert_eq!(g.collect_departed(), 0, "pending operation retains all participants");
        let mut copy = g.clone();
        for (branch, first, expected) in [(&mut g, 2, 6), (&mut copy, 3, 5)] {
            answer(branch, first);
            assert_eq!(branch.counters(target).get("-1/-1"), Some(&expected));
            assert_eq!(branch.players[0].life, 22, "lifelink counts actual damage, not replaced counters");
            assert_eq!(branch.log.iter().skip(from).filter(|e| matches!(e, Event::DamageDealt { .. })).count(), 1);
            assert_eq!(branch.log.iter().skip(from).filter(|e| matches!(e, Event::CountersPut { .. })).count(), 1);
        }
    }
    #[test]
    fn r120_4_spell_and_fight_use_their_retained_resolution_parent() {
        for fight in [false, true] {
            let mut g = game();
            let (source, target) = setup(&mut g);
            let from = g.log.iter().count();
            if fight {
                g.obj_mut(target).chars.pt = Some((1, 20));
                let card = g.put(0, 4, Zone::Hand);
                g.act(0, Move::Cast { card, targets: vec![vec![source.into()], vec![target.into()]] }).unwrap();
            } else {
                g.act(0, Move::Activate { source, ability: 2, targets: vec![vec![target.into()]], paying: vec![] }).unwrap();
            }
            for _ in 0..3 {
                if g.decision.is_some() { break; }
                g.act(g.priority, Move::Pass).unwrap();
            }
            assert!(g.resolving().is_some());
            unchanged(&g, source, target, from);
            let mut copy = g.clone();
            for branch in [&mut g, &mut copy] {
                answer(branch, 2);
                assert_eq!(branch.counters(target).get("-1/-1"), Some(&6));
                assert_eq!(branch.players[0].life, 22);
                assert!(branch.resolving().is_none());
                assert_eq!(branch.obj(source).damage, if fight { 1 } else { 0 });
                assert_eq!(branch.log.iter().skip(from).filter(|e| matches!(e, Event::DamageDealt { .. })).count(), if fight { 2 } else { 1 });
            }
        }
    }
    #[test]
    fn r510_2_combat_aggregates_one_controllers_wither_and_keeps_lifelink_per_source() {
        let mut g = game();
        let (a, target) = setup(&mut g);
        let b = g.put(0, 0, Zone::Battlefield);
        g.put(0, 5, Zone::Battlefield);
        // A zero-power multi-blocker has no division question of its own.
        g.combat.attackers = vec![(a, Target::Player(1)), (b, Target::Player(1))];
        g.combat.blockers = vec![(target, a), (target, b)];
        g.combat.blocking = vec![target];
        g.combat.blocked = vec![a, b];
        g.combat.declared = true;
        g.combat.blockers_declared = true;
        let from = g.log.iter().count();
        g.begin_combat_damage().unwrap();
        unchanged(&g, a, target, from);
        let Some(Decision::Choose { choice: Choice::Replacement { count, .. }, .. }) = &g.decision else { panic!("{:?}", g.decision) };
        assert_eq!(*count, 4, "one simultaneous placement for this source controller");
        let mut copy = g.clone();
        for branch in [&mut g, &mut copy] {
            answer(branch, 2);
            assert_eq!(branch.counters(target).get("-1/-1"), Some(&10));
            let gains: Vec<_> = branch.log.iter().skip(from).filter_map(|e| if let Event::LifeGained { player: 0, amount } = e { Some(*amount) } else { None }).collect();
            assert_eq!(gains, [4, 4]);
            assert_eq!(branch.players[0].life, 28);
            assert_eq!(branch.log.iter().skip(from).filter(|e| matches!(e, Event::DamageDealt { combat: true, .. })).count(), 2);
        }
    }
    #[test]
    fn r120_3d_distinct_source_controllers_make_distinct_placements() {
        let mut g = game();
        let (a, target) = setup(&mut g);
        let b = g.put(2, 0, Zone::Battlefield);
        let (outcome, placements) = Outcome::propose(&g, &[(a, target.into(), 2), (b, target.into(), 2)], false);
        assert_eq!(outcome.hits.len(), 2);
        assert_eq!(placements.iter().map(|p| (p.by, p.count)).collect::<Vec<_>>(), [(0, 2), (2, 2)]);
        g.damage_as_rule(&[(a, target.into(), 2), (b, target.into(), 2)], false).unwrap();
        answer(&mut g, 2);
        assert_eq!(g.players[0].life, 20, "second placement still waiting, no first partial commitment");
        assert!(g.counters(target).is_empty());
        answer(&mut g, 2);
        assert_eq!(g.counters(target).get("-1/-1"), Some(&12));
        assert_eq!((g.players[0].life, g.players[2].life), (22, 22));
    }
    #[test]
    fn r120_4c_lifelink_legality_is_frozen_before_wither_changes_power() {
        let mut g = game();
        let (source, target) = setup(&mut g);
        let opposing = g.put(1, 0, Zone::Battlefield);
        g.put(0, 8, Zone::Battlefield);
        g.damage_as_rule(&[(source, target.into(), 2), (opposing, source.into(), 1)], false).unwrap();
        assert_eq!(g.players[0].life, 20);
        answer(&mut g, 2);
        assert_eq!(g.power(source), Some(1));
        assert_eq!(g.players[0].life, 20, "the prohibition applied before either result was written");
        g.gain_life(0, 1).unwrap();
        assert_eq!(g.players[0].life, 21, "it no longer applies after the completed damage result");
    }

    #[test]
    fn r120_10_actual_simultaneous_hits_share_one_frozen_threshold() {
        // 120.10: aggregate actual simultaneous sources, with noncombat deathtouch
        // reducing lethal to one. Fully prevented deathtouch contributes nothing.
        for protection in [false,true] {
            let defs=vec![
                CardDef::new("Touch","{B}","Creature","Deathtouch",Some((1,1))).unwrap(),
                CardDef::new("Plain","{R}","Creature","",Some((1,1))).unwrap(),
                CardDef::new("Wall","{0}","Creature",if protection {"Protection from black"} else {""},Some((1,5))).unwrap(),
            ];
            let mut g=Game::new(defs,2,20);let touch=g.put(0,0,crate::Zone::Battlefield);
            let plain=g.put(0,1,crate::Zone::Battlefield);let wall=g.put(1,2,crate::Zone::Battlefield);
            let (outcome,placements)=Outcome::propose(&g,&[(touch,wall.into(),1),(plain,wall.into(),4)],false);
            let prepared=g.prepare_counters(placements);let completed=outcome.commit(&mut g,prepared).unwrap();
            assert_eq!(completed.len(),1);
            assert_eq!((completed[0].amount,completed[0].excess),if protection {(4,0)} else {(5,4)});
        }
    }

    #[test]
    fn r120_10_wither_results_do_not_rewrite_the_pre_damage_threshold() {
        let defs=vec![
            CardDef::new("Withering Source","{0}","Creature","Wither",Some((1,1))).unwrap(),
            CardDef::new("Wall","{0}","Creature","Indestructible",Some((1,5))).unwrap(),
        ];
        let mut g=Game::new(defs,2,20);let source=g.put(0,0,crate::Zone::Battlefield);let wall=g.put(1,1,crate::Zone::Battlefield);
        let (outcome,placements)=Outcome::propose(&g,&[(source,wall.into(),6)],false);
        let prepared=g.prepare_counters(placements);let completed=outcome.commit(&mut g,prepared).unwrap();
        assert_eq!((completed[0].amount,completed[0].excess),(6,1));
        assert_eq!(g.counters(wall).get("-1/-1"),Some(&6));
        // The completed record is unchanged by a subsequent departure/new incarnation.
        g.exile(wall);assert!(!g.battlefield.contains(&wall));
        assert_eq!(completed[0].recipient,wall.into());assert_eq!(completed[0].excess,1);
    }

    #[test]
    fn r610_3_pending_return_waits_for_damage_results_and_capture_is_once() {
        let mut g = game();
        let (source, target) = setup(&mut g);
        let prison = g.put(0, 6, Zone::Battlefield);
        let captive = g.put(2, 1, Zone::Battlefield);
        g.put(0, 7, Zone::Battlefield);
        let exiled = g.exile_until(&[captive], prison, None, None)[0].1;
        // Depart directly without the ordinary check that would start the pending return.
        g.move_object(prison, Zone::Graveyard);
        let from = g.log.iter().count();
        g.damage(source, target.into(), 2);
        assert_eq!(g.obj(exiled).zone, Zone::Exile);
        unchanged(&g, source, target, from);
        let mut copy = g.clone();
        for branch in [&mut g, &mut copy] {
            answer(branch, 2);
            assert_eq!(branch.obj(exiled).zone, Zone::Gone, "ordinary check after completion returns it");
            assert_eq!(branch.battlefield.iter().filter(|o| branch.obj(**o).card == Some(1)).count(), 2);
            assert_eq!(branch.counters(target).get("-1/-1"), Some(&6));
            assert_eq!(branch.stack.len(), 1, "one capture of the damage subscription");
            assert_eq!(branch.log.iter().skip(from).filter(|e| matches!(e, Event::DamageDealt { .. })).count(), 1);
        }
    }
}

#[cfg(test)]
mod infect_tests {
    use super::*;
    use crate::state::{Answer, Choice, Decision};
    use crate::{CardDef, Move, Zone};
    fn creature(name: &str, text: &str, pt: (i32, i32)) -> CardDef {
        CardDef::new(name, "{0}", "Creature", text, Some(pt)).unwrap()
    }
    fn game() -> Game {
        let plus = "If one or more energy counters would be put on a player, that many plus one energy counters are put on that player instead.";
        let mut defs = vec![
            creature("Dealer", "Infect\nLifelink", (1, 9)),
            creature("Ordinary", "", (2, 9)),
            creature("Wall", "", (8, 20)),
            CardDef::new("PlusA", "{0}", "Enchantment", plus, None).unwrap(),
            CardDef::new("PlusB", "{0}", "Enchantment", plus, None).unwrap(),
            CardDef::new(
                "DoubleLife",
                "{0}",
                "Enchantment",
                "If you would gain life, you gain twice that much life instead.",
                None,
            )
            .unwrap(),
            creature(
                "Both",
                "Infect\nInfect\nWither\nLifelink\nDeathtouch",
                (1, 9),
            ),
            CardDef::new(
                "Watcher",
                "{0}",
                "Enchantment",
                "Whenever a creature with power 2 or greater is dealt damage, you gain 1 life.",
                None,
            )
            .unwrap(),
            CardDef::new(
                "Guard",
                "{0}",
                "Enchantment",
                "As long as you control a creature with power 2 or greater, you can't gain life.",
                None,
            )
            .unwrap(),
        ];
        for def in &defs {
            crate::play::check(def).unwrap();
        }
        // Typed runtime poison replacement fixtures; no replacement-text admission claim.
        for def in &mut defs[3..5] {
            let crate::replace::Replacement::Counters { kinds: crate::replace::CounterScope::Named(kind), .. } =
                def.replacements[0].result.as_mut().unwrap()
            else {
                panic!("typed fixture")
            };
            *kind = "poison".into();
        }
        Game::new(defs, 3, 20)
    }
    fn poison(g: &Game, from: usize) -> Vec<(Target, u32)> {
        g.log
            .iter()
            .skip(from)
            .filter_map(|e| match e {
                Event::CountersPut {
                    on, kind, count, ..
                } if kind == "poison" => Some((*on, *count)),
                _ => None,
            })
            .collect()
    }
    #[test]
    fn same_controller_groups_poison_once_and_distinct_controllers_keep_placements() {
        for separate in [false, true] {
            let mut g = game();
            let a = g.put(0, 0, Zone::Battlefield);
            let b = g.put(if separate { 2 } else { 0 }, 0, Zone::Battlefield);
            g.put(0, 3, Zone::Battlefield);
            let from = g.log.len();
            let (outcome, p) = Outcome::propose(
                &g,
                &[(a, Target::Player(1), 1), (b, Target::Player(1), 2)],
                true,
            );
            assert_eq!(outcome.hits.len(), 2);
            assert_eq!(
                p.iter().map(|p| (p.by, p.count)).collect::<Vec<_>>(),
                if separate {
                    vec![(0, 1), (2, 2)]
                } else {
                    vec![(0, 3)]
                }
            );
            g.damage_as_rule(
                &[(a, Target::Player(1), 1), (b, Target::Player(1), 2)],
                true,
            ).unwrap();
            assert_eq!(g.players[1].life, 20);
            assert!(
                !g.log
                    .iter()
                    .skip(from)
                    .any(|e| matches!(e, Event::LifeLost { .. }))
            );
            assert_eq!(
                poison(&g, from),
                if separate {
                    vec![(Target::Player(1), 2), (Target::Player(1), 3)]
                } else {
                    vec![(Target::Player(1), 4)]
                }
            );
            assert_eq!(
                (g.players[0].life, g.players[2].life),
                if separate { (21, 22) } else { (23, 20) }
            );
            assert_eq!(
                g.log
                    .iter()
                    .skip(from)
                    .filter(|e| matches!(e, Event::DamageDealt { combat: true, .. }))
                    .count(),
                2
            );
        }
    }
    #[test]
    fn two_effects_suspend_every_result_then_clones_commit_once_and_poison_sba() {
        let mut g = game();
        let a = g.put(0, 0, Zone::Battlefield);
        let b = g.put(0, 0, Zone::Battlefield);
        let normal = g.put(0, 1, Zone::Battlefield);
        let wall = g.put(2, 2, Zone::Battlefield);
        let plus_a = g.put(0, 3, Zone::Battlefield);
        let plus_b = g.put(0, 4, Zone::Battlefield);
        g.put(0, 5, Zone::Battlefield);
        g.put_counters(Target::Player(1), "poison", 5);
        let from = g.log.len();
        g.damage_as_rule(
            &[
                (a, Target::Player(1), 1),
                (b, Target::Player(1), 2),
                (normal, Target::Player(1), 1),
                (a, wall.into(), 1),
            ],
            false,
        ).unwrap();
        let Some(Decision::Choose {
            player: 1,
            choice:
                Choice::Replacement {
                    on: Target::Player(1),
                    count: 3,
                    among,
                    ..
                },
        }) = g.decision.clone()
        else {
            panic!("{:?}", g.decision)
        };
        assert_eq!((g.players[0].life, g.players[1].life), (20, 20));
        assert_eq!(g.obj(wall).damage, 0);
        assert!(g.counters(wall).is_empty());
        assert_eq!(g.counters(Target::Player(1))["poison"], 5);
        assert!(!g.log.iter().skip(from).any(|e| matches!(
            e,
            Event::DamageDealt { .. }
                | Event::LifeLost { .. }
                | Event::LifeGained { .. }
                | Event::CountersPut { .. }
        )));
        assert_eq!(g.collect_departed(), 0);
        let authored = [plus_a, plus_b].map(|source| ReplacementId::from_ability(
            source, crate::binding::AbilityKey::printed(0), 0));
        assert_eq!(among.len(), 2);
        assert!(authored.iter().all(|id| among.contains(id)));
        let before = format!("{g:?}");
        assert_eq!(g.act(0, Move::Choose(Answer::Replacement(authored[0].clone()))),
            Err("that decision is another player's".into()));
        assert_eq!(format!("{g:?}"), before);
        let mut clone = g.clone();
        for (branch, first) in [(&mut g, 0), (&mut clone, 1)] {
            branch.act(1, Move::Choose(Answer::Replacement(authored[first].clone()))).unwrap();
            assert!(branch.decision.is_none());
            assert_eq!(poison(branch, from), vec![(Target::Player(1), 5)]);
            assert!(branch.players[1].lost);
            assert_eq!(branch.players[1].life, 19);
            assert_eq!(branch.players[0].life, 28);
            assert_eq!(branch.counters(wall)["-1/-1"], 1);
            assert_eq!(branch.obj(wall).damage, 0);
            assert_eq!(
                branch
                    .log
                    .iter()
                    .skip(from)
                    .filter(|e| matches!(e, Event::DamageDealt { .. }))
                    .count(),
                4
            );
            assert_eq!(
                branch
                    .log
                    .iter()
                    .skip(from)
                    .filter(|e| matches!(
                        e,
                        Event::LifeLost {
                            player: 1,
                            amount: 1
                        }
                    ))
                    .count(),
                1
            );
            let gains = branch
                .log
                .iter()
                .skip(from)
                .filter_map(|e| match e {
                    Event::LifeGained { player: 0, amount } => Some(*amount),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(gains, vec![4, 4]);
        }
    }
    #[test]
    fn mixed_creature_hits_dual_type_and_duplicate_infect_preserve_other_results() {
        let mut g = game();
        let infect = g.put(0, 6, Zone::Battlefield);
        let ordinary = g.put(0, 1, Zone::Battlefield);
        let wall = g.put(1, 2, Zone::Battlefield);
        g.obj_mut(wall).chars.types.push(CardType::Planeswalker);
        g.put_counters(wall, "loyalty", 8);
        let from = g.log.len();
        g.damage_as_rule(
            &[
                (infect, wall.into(), 2),
                (ordinary, wall.into(), 1),
                (infect, Target::Player(1), 1),
                (ordinary, Target::Player(1), 1),
            ],
            false,
        ).unwrap();
        assert_eq!(g.obj(wall).damage, 1);
        assert!(g.obj(wall).deathtouched);
        assert_eq!(g.counters(wall)["-1/-1"], 2);
        assert_eq!(g.counters(wall)["loyalty"], 5);
        assert_eq!((g.players[0].life, g.players[1].life), (23, 19));
        assert_eq!(poison(&g, from), vec![(Target::Player(1), 1)]);
    }
    #[test]
    fn pre_result_capture_and_frozen_lifelink_survive_infect_counter_changes() {
        let mut g = game();
        let source = g.put(0, 0, Zone::Battlefield);
        let target = g.put(0, 2, Zone::Battlefield);
        g.put(0, 7, Zone::Battlefield);
        g.put(0, 8, Zone::Battlefield);
        // Freeze the prohibition before these counters reduce the controlling creature
        // below its threshold.
        let (outcome, placements) = Outcome::propose(&g, &[(source, target.into(), 7)], false);
        assert_eq!(outcome.gains, vec![(0, 0)]);
        let prepared = g.prepare_counters(placements);
        outcome.commit(&mut g, prepared).unwrap();
        assert_eq!(g.power(target), Some(1));
        assert_eq!(g.players[0].life, 20);
        g.gain_life(0, 1).unwrap();
        assert_eq!(g.players[0].life, 21);
        assert_eq!(
            g.triggered.len(),
            1,
            "damage trigger captured before counter-reduced power"
        );
    }
    #[test]
    fn infect_commander_damage_counts_combat_even_without_life_loss() {
        let leader = CardDef::new(
            "Leader",
            "{0}",
            "Legendary Creature — Human",
            "Infect",
            Some((3, 3)),
        )
        .unwrap();
        crate::play::check(&leader).unwrap();
        let mut g = Game::new(vec![leader], 2, 40);
        let card = g.put(0, 0, Zone::Command);
        g.designate_commander(card).unwrap();
        g.move_object(card, Zone::Battlefield);
        let source = g.battlefield[0];
        g.damage_as_rule(&[(source, Target::Player(1), 3)], true).unwrap();
        g.damage(source, Target::Player(1), 4);
        assert_eq!(g.commanders[0].damage, vec![0, 3]);
        assert_eq!(g.players[1].life, 40);
        assert_eq!(g.counters(Target::Player(1))["poison"], 7);
    }
}

#[cfg(test)]
mod toxic_tests {
    use super::*;
    use crate::state::{Answer, Choice, Decision};
    use crate::{CardDef, Move, Zone};
    fn creature(name: &str, text: &str) -> CardDef {
        CardDef::new(name, "{0}", "Creature", text, Some((3, 5))).unwrap()
    }
    fn game() -> Game {
        let defs = vec![
            creature("Poisoner", "Toxic 2\nLifelink"),
            creature("Mixed Poisoner", "Infect\nToxic 1\nLifelink"),
            CardDef::new("Plus", "{0}", "Enchantment", "If one or more poison counters would be put on a player, that many plus one poison counters are put on that player instead.", None).unwrap(),
            CardDef::new("Times", "{0}", "Enchantment", "If one or more poison counters would be put on a player, twice that many poison counters are put on that player instead.", None).unwrap(),
        ];
        for d in &defs { crate::play::check(d).unwrap(); }
        // These fixed player-counter replacements are admitted text; only the simultaneous
        // damage vector below is an internal fixture rather than combat announcement.
        Game::new(defs, 3, 20)
    }
    #[test]
    fn simultaneous_infect_and_toxic_share_each_controllers_player_placement() {
        for different in [false, true] {
            let mut g = game();
            let toxic = g.put(0, 0, Zone::Battlefield);
            let infect = g.put(if different { 2 } else { 0 }, 1, Zone::Battlefield);
            let (outcome, placements) = Outcome::propose(&g, &[(toxic, Target::Player(1), 3), (infect, Target::Player(1), 3), (toxic, Target::Player(2), 1)], true);
            let expected = if different { vec![(1, 0, 2), (1, 2, 4), (2, 0, 2)] } else { vec![(1, 0, 6), (2, 0, 2)] };
            assert_eq!(placements.iter().map(|p| { let Target::Player(player) = p.on else {panic!("player")}; (player, p.by, p.count) }).collect::<Vec<_>>(), expected);
            let prepared = g.prepare_counters(placements);
            outcome.commit(&mut g, prepared).unwrap();
            assert_eq!((g.players[1].life, g.players[2].life), (17, if different {22} else {19}));
            assert_eq!(g.counters(Target::Player(1))["poison"], 6);
            assert_eq!(g.counters(Target::Player(2))["poison"], 2);
            assert_eq!(g.log.iter().filter(|e| matches!(e, Event::DamageDealt { .. })).count(), 3);
            assert_eq!(g.players[0].life, if different {24} else {27});
        }
    }
    #[test]
    fn poison_choices_freeze_all_damage_results_and_clones_commit_once_before_poison_loss() {
        let mut g = game();
        let toxic = g.put(0, 0, Zone::Battlefield);
        let infect = g.put(0, 1, Zone::Battlefield);
        let plus = g.put(1, 2, Zone::Battlefield);
        let times = g.put(1, 3, Zone::Battlefield);
        g.put_counters(Target::Player(1), "poison", 3);
        let from = g.log.len();
        g.damage_as_rule(&[(toxic, Target::Player(1), 3), (infect, Target::Player(1), 3)], true).unwrap();
        let Some(Decision::Choose { player:1, choice:Choice::Replacement { count:6, on:Target::Player(1), among, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision); };
        assert_eq!((g.players[0].life, g.players[1].life), (20, 20));
        assert_eq!(g.counters(Target::Player(1))["poison"], 3);
        assert!(!g.players[1].lost);
        assert_eq!(g.log.len(), from);
        let authored = [plus, times].map(|source| ReplacementId::from_ability(
            source, crate::binding::AbilityKey::printed(0), 0));
        assert_eq!(among.len(), 2);
        assert!(authored.iter().all(|id| among.contains(id)));
        let before = format!("{g:?}");
        assert_eq!(g.act(0, Move::Choose(Answer::Replacement(authored[0].clone()))),
            Err("that decision is another player's".into()));
        assert_eq!(format!("{g:?}"), before);
        assert_eq!(g.collect_departed(), 0);
        let mut clone = g.clone();
        for (branch, first, expected) in [(&mut g, 0, 14), (&mut clone, 1, 13)] {
            let chosen = authored[first].clone();
            branch.act(1, Move::Choose(Answer::Replacement(chosen))).unwrap();
            assert!(branch.decision.is_none());
            assert_eq!((branch.players[0].life, branch.players[1].life), (26, 17));
            assert!(branch.players[1].lost, "SBA only after the whole operation completes");
            let counts = branch.log.iter().skip(from).filter_map(|e| match e { Event::CountersPut { kind, count, .. } if kind == "poison" => Some(*count), _ => None }).collect::<Vec<_>>();
            assert_eq!(counts, [expected]);
            assert_eq!(branch.log.iter().skip(from).filter(|e| matches!(e, Event::DamageDealt { .. })).count(), 2);
            assert_eq!(branch.log.iter().skip(from).filter(|e| matches!(e, Event::LifeLost { .. })).count(), 1);
        }
    }
    #[test]
    fn zero_hits_and_noncombat_toxic_preserve_infect_and_no_counter_damage_to_creatures() {
        let mut g = game();
        let source = g.put(0, 0, Zone::Battlefield);
        let infect = g.put(0, 1, Zone::Battlefield);
        let wall = g.put(1, 0, Zone::Battlefield);
        let (_, placements) = Outcome::propose(&g, &[(source, Target::Player(1), 0), (infect, Target::Player(1), 0)], true);
        assert!(placements.is_empty());
        let (_, placements) = Outcome::propose(&g, &[(source, Target::Player(1), 3), (infect, Target::Player(1), 3)], false);
        assert_eq!(placements.len(), 1);
        assert_eq!(placements[0].count, 3, "noncombat Infect without Toxic");
        let (outcome, placements) = Outcome::propose(&g, &[(source, wall.into(), 2)], true);
        assert!(placements.is_empty());
        let prepared = g.prepare_counters(placements);
        outcome.commit(&mut g, prepared).unwrap();
        assert_eq!(g.obj(wall).damage, 2);
        assert!(g.counters(wall).is_empty());
    }
}

/// One modifier on one simultaneous source-recipient damage amount. The source is display information; identity
/// is the effect instance plus the operation-local hit, never the displayed source alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub effect: crate::operation::ReplacementId,
    pub hit: usize,
    pub source: ObjId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProposedHit {
    pub id: usize,
    pub source: ObjId,
    pub amount: u32,
}

/// Damage-only identities: protection is structural, shields span a recipient group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DamageOption {
    Modifier(Offer),
    Protection {
        hit: usize,
        source: ObjId,
    },
    Prevention {
        effect: crate::operation::ReplacementId,
        hit: usize,
        source: ObjId,
    },
    Shield(crate::operation::ReplacementId),
}
impl DamageOption {
    pub fn trace(&self, visit: &mut dyn FnMut(ObjId)) {
        match self {
            Self::Modifier(o) => {
                visit(o.effect.source);
                visit(o.source);
            }
            Self::Protection { source, .. } => visit(*source),
            Self::Prevention { effect, source, .. } => {
                visit(effect.source);
                visit(*source);
            }
            Self::Shield(id) => visit(id.source),
        }
    }
}
#[derive(Debug, Clone)]
enum Question {
    Order(Vec<DamageOption>),
    Allocation {
        shield: crate::operation::ReplacementId,
        total: u32,
        hits: Vec<ProposedHit>,
    },
}
#[derive(Debug, Clone)]
enum Edit {
    Amount(crate::replace::CounterChange),
    All,
    Shield(crate::operation::ReplacementId),
}
/// Validate before consuming a pending allocation (615.7).
pub fn valid_allocation(total: u32, hits: &[ProposedHit], allocation: &[(usize, u32)]) -> bool {
    allocation.len() == hits.len()
        && allocation.iter().enumerate().all(|(i, (id, n))| {
            !allocation[..i].iter().any(|(earlier, _)| earlier == id)
                && hits.iter().any(|h| h.id == *id && *n <= h.amount)
        })
        && allocation.iter().map(|(_, n)| u64::from(*n)).sum::<u64>() == u64::from(total)
}

#[derive(Debug, Clone)]
struct Proposal {
    id: usize,
    source: ObjId,
    to: Target,
    amount: u32,
    applied: Vec<crate::operation::ReplacementId>,
    prevented: Vec<DamageOption>,
}

/// Amount replacement operates on each hit, with choices grouped by recipient (616.1).
#[derive(Debug, Clone)]
pub(crate) struct Proposals {
    combat: bool,
    /// "The damage can't be prevented" is said of these hits (615.12).
    unpreventable: bool,
    hits: Vec<Proposal>,
    recipients: Vec<Target>,
    at: usize,
    asked: Option<Question>,
    used: Vec<(ReplacementId, u32)>,
    pub(crate) choices: Vec<crate::state::PublicChoice>,
}
impl Proposals {
    pub(crate) fn new(game: &Game, damage: &[(ObjId, Target, u32)], combat: bool) -> crate::Res<Self> {
        // 608.2f/700.1: one simultaneous source-recipient happening may have several
        // expression fragments. Combine them before a replacement gets its one
        // opportunity (614.5), or a finite shield counts sources (615.7).
        // Keep the first fragment's operation-local identity; distinct sources,
        // recipients and later damage operations remain distinct.
        let mut hits: Vec<Proposal> = Vec::new();
        for (id, &(source, to, amount)) in damage.iter().enumerate() {
            if amount == 0 || !game.takes(Verb::DealDamage, Role::To, to) { continue; }
            if let Some(hit) = hits.iter_mut().find(|hit| hit.source == source && hit.to == to) {
                hit.amount = hit.amount.checked_add(amount).ok_or_else(|| crate::Unsupported(
                    "simultaneous damage from one source to one recipient exceeds the engine numerical range".into()))?;
            } else {
                hits.push(Proposal { id, source, to, amount, applied: Vec::new(), prevented: Vec::new() });
            }
        }
        let mut recipients = Vec::new();
        for hit in &hits {
            if !recipients.contains(&hit.to) {
                recipients.push(hit.to);
            }
        }
        recipients.sort_by_key(|to| {
            (Self::chooser(game, *to) + game.players.len() - game.active) % game.players.len()
        });
        Ok(Self {
            combat,
            unpreventable: false,
            hits,
            recipients,
            at: 0,
            asked: None,
            used: Vec::new(),
            choices: Vec::new(),
        })
    }
    fn chooser(game: &Game, on: Target) -> PlayerId {
        match on {
            Target::Player(p) => p,
            Target::Object(o) => game.controller(o),
        }
    }
    fn modifier_options(&self, game: &Game) -> Vec<(Offer, crate::replace::CounterChange)> {
        let Some(on) = self.recipients.get(self.at) else {
            return Vec::new();
        };
        let mut options = Vec::new();
        for hit in self.hits.iter().filter(|h| h.to == *on && h.amount > 0) {
            for &provider in &game.battlefield {
                let ctx = crate::select::Ctx::new(game.controller(provider), provider);
                for (key, r) in game.replacements(provider) {
                    let id = crate::operation::ReplacementId::from_ability(provider, key.clone(), 0);
                    if hit.applied.contains(&id) {
                        continue;
                    }
                    if let crate::replace::Replacement::Damage {
                        from,
                        to,
                        kind,
                        change,
                    } = r
                        && !matches!(
                            (kind, self.combat),
                            (ir::event::DamageKind::Combat, false)
                                | (ir::event::DamageKind::Noncombat, true)
                        )
                        && from.matches(game, &ctx.clone().with_key(key.clone()), hit.source)
                        && to.iter().any(|n| n.is(game, &ctx.clone().with_key(key.clone()), hit.to))
                    {
                        options.push((
                            Offer {
                                effect: id,
                                hit: hit.id,
                                source: hit.source,
                            },
                            *change,
                        ));
                    }
                }
            }
        }
        options
    }
    fn shield_hits(&self, game: &Game, id: ReplacementId) -> Vec<ProposedHit> {
        let on = self.recipients[self.at];
        let effect = game
            .prevention
            .iter()
            .find(|e| e.id == id)
            .expect("retained shield");
        self.hits
            .iter()
            .filter(|h| {
                h.to == on
                    && h.amount > 0
                    && !h.prevented.contains(&DamageOption::Shield(id.clone()))
                    && effect.matches(game, h.source, h.to, self.combat)
            })
            .map(|h| ProposedHit {
                id: h.id,
                source: h.source,
                amount: h.amount,
            })
            .collect()
    }
    fn options(&self, game: &Game) -> Vec<(DamageOption, Edit)> {
        let Some(&on) = self.recipients.get(self.at) else {
            return Vec::new();
        };
        let mut options: Vec<_> = self
            .modifier_options(game)
            .into_iter()
            .map(|(offer, change)| (DamageOption::Modifier(offer), Edit::Amount(change)))
            .collect();
        if self.unpreventable || game.unpreventable() {
            return options;
        }
        for hit in self.hits.iter().filter(|h| h.to == on && h.amount > 0) {
            let protection = DamageOption::Protection {
                hit: hit.id,
                source: hit.source,
            };
            if !hit.prevented.contains(&protection)
                && game.protected_from(
                    hit.to,
                    &crate::select::Ctx::new(game.controller(hit.source), hit.source),
                )
            {
                options.push((protection, Edit::All));
            }
            for effect in &game.prevention {
                if effect.remaining.is_none()
                    && effect.matches(game, hit.source, hit.to, self.combat)
                {
                    let offer = DamageOption::Prevention {
                        effect: effect.id.clone(),
                        hit: hit.id,
                        source: hit.source,
                    };
                    if !hit.prevented.contains(&offer) {
                        options.push((offer, Edit::All));
                    }
                }
            }
            // a permanent's "prevent all damage that would be dealt to …" (615.1)
            for effect in game.static_preventions(hit.source, hit.to, self.combat) {
                let offer = DamageOption::Prevention { effect, hit: hit.id, source: hit.source };
                if !hit.prevented.contains(&offer) {
                    options.push((offer, Edit::All));
                }
            }
        }
        for effect in &game.prevention {
            if let Some(remaining) = effect.remaining {
                let used = self
                    .used
                    .iter()
                    .find(|(id, _)| *id == effect.id)
                    .map_or(0, |(_, n)| *n);
                if remaining > used && !self.shield_hits(game, effect.id.clone()).is_empty() {
                    options.push((DamageOption::Shield(effect.id.clone()), Edit::Shield(effect.id.clone())));
                }
            }
        }
        options
    }
    fn touches(&self, game: &Game, offer: DamageOption, hit: usize) -> bool {
        match offer {
            DamageOption::Modifier(o) => o.hit == hit,
            DamageOption::Protection { hit: id, .. } | DamageOption::Prevention { hit: id, .. } => {
                id == hit
            }
            DamageOption::Shield(id) => self.shield_hits(game, id).iter().any(|h| h.id == hit),
        }
    }
    fn allocate(&mut self, id: ReplacementId, hits: &[ProposedHit], allocation: &[(usize, u32)]) {
        let mut total = 0;
        for offered in hits {
            let amount = allocation
                .iter()
                .find(|(hit, _)| *hit == offered.id)
                .expect("validated allocation")
                .1;
            let hit = self.hits.iter_mut().find(|h| h.id == offered.id).unwrap();
            hit.amount -= amount;
            hit.prevented.push(DamageOption::Shield(id.clone()));
            total += amount;
        }
        if let Some((_, used)) = self.used.iter_mut().find(|(shield, _)| *shield == id) {
            *used += total;
        } else {
            self.used.push((id, total));
        }
    }
    fn apply_option(
        &mut self,
        game: &Game,
        offer: DamageOption,
        edit: Edit,
    ) -> crate::Res<Option<crate::state::Decision>> {
        match edit {
            Edit::Amount(change) => {
                let DamageOption::Modifier(o) = offer else {
                    unreachable!()
                };
                self.apply(o, change)?;
            }
            Edit::All => {
                let hit = match offer {
                    DamageOption::Protection { hit, .. } | DamageOption::Prevention { hit, .. } => {
                        hit
                    }
                    _ => unreachable!(),
                };
                let h = self.hits.iter_mut().find(|h| h.id == hit).unwrap();
                h.amount = 0;
                h.prevented.push(offer);
            }
            Edit::Shield(id) => {
                let hits = self.shield_hits(game, id.clone());
                let effect = game.prevention.iter().find(|e| e.id == id).unwrap();
                let used = self
                    .used
                    .iter()
                    .find(|(shield, _)| *shield == id)
                    .map_or(0, |(_, n)| *n);
                let remaining = effect.remaining.unwrap() - used;
                let sum: u64 = hits.iter().map(|h| u64::from(h.amount)).sum();
                let total = u64::from(remaining).min(sum) as u32;
                if hits.len() > 1 && u64::from(total) < sum {
                    self.asked = Some(Question::Allocation {
                        shield: id.clone(),
                        total,
                        hits: hits.clone(),
                    });
                    let on = self.recipients[self.at];
                    return Ok(Some(crate::state::Decision::Choose {
                        player: Self::chooser(game, on),
                        choice: crate::state::Choice::Prevention {
                            on,
                            shield: id,
                            total,
                            hits,
                        },
                    }));
                }
                let mut unspent = total;
                let allocation: Vec<_> = hits.iter().map(|h| {
                    let amount = h.amount.min(unspent);
                    unspent -= amount;
                    (h.id, amount)
                }).collect();
                self.allocate(id, &hits, &allocation);
            }
        }
        Ok(None)
    }
    fn apply(&mut self, offer: Offer, change: crate::replace::CounterChange) -> crate::Res<()> {
        let hit = self
            .hits
            .iter_mut()
            .find(|h| h.id == offer.hit)
            .expect("offered hit");
        hit.amount = change.apply(hit.amount)?;
        hit.applied.push(offer.effect);
        Ok(())
    }
    pub(crate) fn waiting(&self) -> bool {
        self.asked.is_some()
    }
    pub(crate) fn advance(&mut self, game: &Game) -> crate::Res<Option<crate::state::Decision>> {
        while let Some(&on) = self.recipients.get(self.at) {
            let options = self.options(game);
            if options.is_empty() {
                self.at += 1;
                continue;
            }
            // Prevent-all commutes with pure amounts when no consumable shield touches
            // that hit. Keep the established protection fast path in that bounded case.
            let harmless_all = options.iter().find(|(o, e)| {
                matches!(e, Edit::All)
                    && !options.iter().any(|(other, _)| {
                        matches!(other, DamageOption::Shield(_))
                            && self.hits.iter().any(|h| {
                                self.touches(game, o.clone(), h.id) && self.touches(game, other.clone(), h.id)
                            })
                    })
            });
            if let Some((offer, edit)) = harmless_all {
                self.apply_option(game, offer.clone(), edit.clone())?;
                continue;
            }
            // An option is automatic only when no other option touches any of its hits.
            // A shared shield is one group option but competes on every eligible hit.
            let unopposed = options.iter().find(|(offer, _)| {
                self.hits
                    .iter()
                    .filter(|h| self.touches(game, offer.clone(), h.id))
                    .all(|h| {
                        options
                            .iter()
                            .filter(|(o, _)| self.touches(game, o.clone(), h.id))
                            .count()
                            == 1
                    })
            });
            if let Some((offer, edit)) = unopposed {
                if let Some(d) = self.apply_option(game, offer.clone(), edit.clone())? {
                    return Ok(Some(d));
                }
                continue;
            }
            let among: Vec<_> = options.into_iter().map(|(o, _)| o).collect();
            self.asked = Some(Question::Order(among.clone()));
            let hits = self
                .hits
                .iter()
                .filter(|h| h.to == on)
                .map(|h| ProposedHit {
                    id: h.id,
                    source: h.source,
                    amount: h.amount,
                })
                .collect();
            let choice = if among.iter().all(|o| matches!(o, DamageOption::Modifier(_))) {
                crate::state::Choice::DamageReplacement {
                    on,
                    hits,
                    among: among
                        .into_iter()
                        .map(|o| match o {
                            DamageOption::Modifier(o) => o,
                            _ => unreachable!(),
                        })
                        .collect(),
                }
            } else {
                crate::state::Choice::DamageOrder { on, hits, among }
            };
            return Ok(Some(crate::state::Decision::Choose {
                player: Self::chooser(game, on),
                choice,
            }));
        }
        Ok(None)
    }
    pub(crate) fn answer(
        &mut self,
        game: &Game,
        player: PlayerId,
        answer: crate::state::Answer,
    ) -> crate::Res<Option<crate::state::Decision>> {
        let on = self.recipients[self.at];
        match self.asked.take().expect("asked damage") {
            Question::Order(among) => {
                let offer = match answer {
                    crate::state::Answer::DamageReplacement(o) => DamageOption::Modifier(o),
                    crate::state::Answer::DamageOrder(o) => o,
                    _ => unreachable!("validated order"),
                };
                assert!(among.contains(&offer));
                let (_, edit) = self
                    .options(game)
                    .into_iter()
                    .find(|(o, _)| *o == offer)
                    .expect("offered effect");
                match offer {
                    DamageOption::Modifier(ref o) => {
                        self.choices
                            .push(crate::state::PublicChoice::DamageReplacement {
                                player,
                                on,
                                offer: o.clone(),
                            })
                    }
                    _ => self.choices.push(crate::state::PublicChoice::DamageOrder {
                        player,
                        on,
                        offer: offer.clone(),
                    }),
                };
                if let Some(d) = self.apply_option(game, offer, edit)? {
                    return Ok(Some(d));
                }
            }
            Question::Allocation {
                shield,
                total,
                hits,
            } => {
                let crate::state::Answer::Prevention(allocation) = answer else {
                    unreachable!("validated allocation")
                };
                assert!(valid_allocation(total, &hits, &allocation));
                let allocation: Vec<_> = hits
                    .iter()
                    .map(|h| {
                        (
                            h.id,
                            allocation.iter().find(|(id, _)| *id == h.id).unwrap().1,
                        )
                    })
                    .collect();
                self.allocate(shield.clone(), &hits, &allocation);
                self.choices.push(crate::state::PublicChoice::Prevention {
                    player,
                    on,
                    shield,
                    allocation,
                });
            }
        }
        self.advance(game)
    }
    pub(crate) fn freeze(
        self,
        game: &Game,
    ) -> crate::Res<(Outcome, Vec<Placement>, Vec<(ReplacementId, u32)>)> {
        let damage = self
            .hits
            .into_iter()
            .map(|h| (h.id, h.source, h.to, h.amount))
            .collect::<Vec<_>>();
        let (outcome, placements) = Outcome::propose_hits(game, &damage, self.combat)?;
        Ok((outcome, placements, self.used))
    }
}

#[derive(Debug, Clone)]
enum Stage {
    Proposed(Proposals),
    Results {
        outcome: Outcome,
        ordering: crate::operation::Ordering<crate::operation::CounterEvent>,
    },
    Finished,
}
#[derive(Debug, Clone)]
pub(crate) struct Dealing {
    origin: crate::operation::EventOrigin,
    stage: Stage,
    used: Vec<(ReplacementId, u32)>,
    choices: Vec<crate::state::PublicChoice>,
}
impl Dealing {
    pub(crate) fn new(game: &Game, damage: &[(ObjId, Target, u32)], combat: bool, origin: crate::operation::EventOrigin) -> crate::Res<Self> {
        Ok(Self {
            origin,
            stage: Stage::Proposed(Proposals::new(game, damage, combat)?),
            used: Vec::new(),
            choices: Vec::new(),
        })
    }
    /// This damage can't be prevented (615.12).
    pub(crate) fn unpreventable(mut self) -> Self {
        if let Stage::Proposed(p) = &mut self.stage { p.unpreventable = true; }
        self
    }
    pub(crate) fn waiting(&self) -> bool {
        match &self.stage {
            Stage::Proposed(p) => p.waiting(),
            Stage::Results { ordering, .. } => ordering.waiting(),
            Stage::Finished => false,
        }
    }
    pub(crate) fn choices(&self) -> impl Iterator<Item = &crate::state::PublicChoice> {
        let current: &[_] = match &self.stage {
            Stage::Proposed(p) => &p.choices,
            Stage::Results { ordering, .. } => &ordering.choices,
            Stage::Finished => &[],
        };
        self.choices.iter().chain(current)
    }
    pub(crate) fn advance(&mut self, game: &mut Game) -> crate::Res<Option<crate::state::Decision>> {
        if let Stage::Proposed(p) = &mut self.stage {
            if let Some(d) = p.advance(game)? {
                return Ok(Some(d));
            }
            let Stage::Proposed(p) = std::mem::replace(&mut self.stage, Stage::Finished) else {
                unreachable!()
            };
            self.choices.extend(p.choices.clone());
            let (outcome, placements, used) = p.freeze(game)?;
            self.used = used;
            let ordering = crate::operation::Ordering::new(game,
                crate::operation::CounterEvent::batch(placements,self.origin)?);
            self.stage = Stage::Results { outcome, ordering };
        }
        let Stage::Results { ordering, .. } = &mut self.stage else {
            unreachable!("completed damage")
        };
        ordering.advance(game)
    }
    pub(crate) fn answer(
        &mut self,
        game: &mut Game,
        player: PlayerId,
        answer: crate::state::Answer,
    ) -> crate::Res<Option<crate::state::Decision>> {
        match &mut self.stage {
            Stage::Proposed(p) => p.answer(game, player, answer),
            Stage::Results { ordering, .. } => ordering.answer(game, player, answer),
            Stage::Finished => unreachable!(),
        }
    }
    pub(crate) fn commit(self, game: &mut Game) -> crate::Res<Vec<CompletedDamage>> {
        let Stage::Results { outcome, ordering } = self.stage else {
            unreachable!("unfinished damage")
        };
        let prepared = ordering.prepare_counters(game);
        prepared.check(game)?;
        game.commit_prevention(&self.used);
        outcome.commit(game, prepared)
    }
}

#[cfg(test)]
mod modifier_tests {
    use super::*;
    use crate::state::{Answer, Choice, Decision};
    use crate::{CardDef, Move, Zone};
    fn game() -> Game {
        let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        let catalog = CardDef::catalog(&faces);
        let mut defs = [
            "Furnace of Rath",
            "Torbran, Thane of Red Fell",
            "Embermaw Hellion",
        ]
        .into_iter()
        .map(|n| CardDef::from_faces(catalog.get(n).unwrap()).unwrap())
        .collect::<Vec<_>>();
        defs.push(CardDef::new("Red", "{R}", "Creature", "", Some((3, 30))).unwrap());
        defs.push(CardDef::new("Wall", "{0}", "Creature", "", Some((0, 40))).unwrap());
        for d in &defs {
            crate::play::check(d).unwrap();
        }
        Game::new(defs, 3, 20)
    }
    fn choose(g: &mut Game, player: PlayerId, provider: ObjId, source: ObjId, hit: usize) {
        let Some(Decision::Choose {
            player: chooser,
            choice: Choice::DamageReplacement { among, .. },
        }) = g.decision.clone() else { panic!("{:?}", g.decision) };
        assert_eq!(chooser, player);
        let offer = Offer { effect: ReplacementId::from_ability(
            provider, crate::binding::AbilityKey::printed(0), 0), hit, source };
        assert!(among.contains(&offer), "authored hit/effect is offered");
        g.act(player, Move::Choose(Answer::DamageReplacement(offer))).unwrap();
    }
    /// Rules: 608.2f, 614.5, 616.1. Duplicate fragments are one source-recipient event.
    #[test]
    fn r616_1_same_source_recipient_fragments_share_order_and_cloned_pending_state() {
        let mut g=game();
        let furnace=g.put(0,0,Zone::Battlefield);
        let torbran=g.put(0,1,Zone::Battlefield);
        let source=g.put(0,3,Zone::Battlefield);
        let start=g.log.len();
        g.damage_as_rule(&[(source,Target::Player(1),3),(source,Target::Player(1),3)],false).unwrap();
        let Some(Decision::Choose{player:1,choice:Choice::DamageReplacement{among,hits,..}})=g.decision.clone() else {panic!("damage ordering")};
        assert_eq!(among.len(),2);
        assert_eq!(hits,vec![ProposedHit{id:0,source,amount:6}]);
        assert_eq!(g.players[1].life,20);
        assert_eq!(g.log.len(),start);
        let authored=Offer{effect:ReplacementId::from_ability(furnace,crate::binding::AbilityKey::printed(0),0),hit:0,source};
        assert!(among.contains(&authored));
        let before=format!("{g:?}");
        assert_eq!(g.act(0,Move::Choose(Answer::DamageReplacement(authored.clone()))),Err("that decision is another player's".into()));
        assert_eq!(format!("{g:?}"),before);
        let mut stale=authored;stale.hit=999;
        assert_eq!(g.act(1,Move::Choose(Answer::DamageReplacement(stale))),Err("choose an offered effect and damage hit (616.1)".into()));
        assert_eq!(format!("{g:?}"),before);
        let mut copy=g.clone();
        choose(&mut g,1,furnace,source,0); // 6*2+2=14.
        choose(&mut copy,1,torbran,source,0); // (6+2)*2=16.
        for (branch,life,amount) in [(&g,6,14),(&copy,4,16)] {
            assert!(branch.decision.is_none());
            assert_eq!(branch.players[1].life,life);
            assert_eq!(branch.log.iter().skip(start).filter_map(|e|match e{Event::DamageDealt{amount,..}=>Some(*amount),_=>None}).collect::<Vec<_>>(),vec![amount]);
        }
    }

    /// Rules: 608.2c/f, 700.1. Preserve different sources/recipients and separate events.
    #[test]
    fn r120_4_damage_normalization_keeps_distinct_sources_recipients_and_operations() {
        for distinct_sources in [false,true] {
            let mut g=game();
            let a=g.put(0,3,Zone::Battlefield);let b=g.put(0,3,Zone::Battlefield);
            g.damage_as_rule(&[(a,Target::Player(1),3),(if distinct_sources{b}else{a},Target::Player(1),3)],false).unwrap();
            assert_eq!(g.players[1].life,14);
            let amounts:Vec<_>=g.log.iter().filter_map(|e|match e{Event::DamageDealt{amount,..}=>Some(*amount),_=>None}).collect();
            assert_eq!(amounts,if distinct_sources{vec![3,3]}else{vec![6]});
        }
        let mut g=game();let a=g.put(0,3,Zone::Battlefield);
        g.damage_as_rule(&[(a,Target::Player(1),3),(a,Target::Player(2),3)],false).unwrap();
        assert_eq!((g.players[1].life,g.players[2].life),(17,17));
        let from=g.log.len();
        g.damage_as_rule(&[(a,Target::Player(1),3)],false).unwrap();
        g.damage_as_rule(&[(a,Target::Player(1),3)],false).unwrap();
        assert_eq!(g.players[1].life,11);
        assert_eq!(g.log.iter().skip(from).filter(|e|matches!(e,Event::DamageDealt{amount:3,..})).count(),2);
    }

    /// CR608.2c: repeated damage instructions in one resolving spell remain separate.
    #[test]
    fn r608_2c_separate_damage_instructions_each_receive_the_amount_modifier() {
        let mut defs = (*game().defs).clone();
        let spell = CardDef::new("Sequential", "{R}", "Sorcery",
            "This spell deals 3 damage to each opponent. This spell deals 3 damage to each opponent.", None).unwrap();
        crate::play::check(&spell).unwrap();
        let card_index = defs.len();
        defs.push(spell);
        let mut g = Game::new(defs, 2, 20);
        g.put(0, 1, Zone::Battlefield); // Torbran: each instruction's 3 becomes 5.
        let card = g.put(0, card_index, Zone::Hand);
        g.players[0].pool.colors[3] = 1;
        let mut payment = crate::state::Pool::default();
        payment.colors[3] = 1;
        g.act(0, Move::Paying(payment, Box::new(Move::Cast { card, targets: vec![] }))).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        assert!(g.decision.is_none());
        assert!(g.stack.is_empty());
        assert_eq!(g.players[1].life, 10);
        assert_eq!(g.log.iter().filter_map(|e| match e {
            Event::DamageDealt { amount, .. } => Some(*amount), _ => None,
        }).collect::<Vec<_>>(), [5, 5]);
    }

    /// Uncited (representation contract): refuse checked aggregate overflow without mutation.
    #[test]
    fn simultaneous_source_recipient_aggregate_overflow_preserves_state() {
        let mut g=game();let source=g.put(0,3,Zone::Battlefield);
        let before=format!("{g:?}");
        assert_eq!(g.damage_as_rule(&[(source,Target::Player(1),u32::MAX),(source,Target::Player(1),1)],false).unwrap_err().0,
            "simultaneous damage from one source to one recipient exceeds the engine numerical range");
        assert_eq!(format!("{g:?}"),before);
    }
    #[test]
    fn r616_1_permanent_controller_and_players_choose_apnap_before_any_commit() {
        let mut g = game();
        let furnace = g.put(0, 0, Zone::Battlefield);
        let torbran = g.put(0, 1, Zone::Battlefield);
        let source = g.put(0, 3, Zone::Battlefield);
        let wall = g.put(2, 4, Zone::Battlefield);
        g.damage_as_rule(
            &[(source, wall.into(), 3), (source, Target::Player(1), 3)],
            false,
        ).unwrap();
        assert!(matches!(
            g.decision,
            Some(Decision::Choose { player: 1, .. })
        ));
        choose(&mut g, 1, furnace, source, 1);
        assert!(matches!(
            g.decision,
            Some(Decision::Choose { player: 2, .. })
        ));
        assert_eq!(g.players[1].life, 20);
        assert_eq!(g.obj(wall).damage, 0);
        choose(&mut g, 2, torbran, source, 0);
        assert_eq!(g.players[1].life, 12);
        assert_eq!(g.obj(wall).damage, 10);
    }
    #[test]
    fn r120_8_zero_does_not_become_damage_and_hellion_excludes_only_itself() {
        let mut g = game();
        let hellion = g.put(0, 2, Zone::Battlefield);
        let red = g.put(0, 3, Zone::Battlefield);
        g.damage(red, Target::Player(1), 0);
        assert_eq!(g.players[1].life, 20);
        assert!(g.decision.is_none());
        g.damage(hellion, Target::Player(1), 3);
        assert_eq!(g.players[1].life, 17);
        g.damage(red, Target::Player(1), 3);
        assert_eq!(g.players[1].life, 13);
    }
    #[test]
    fn r120_4_proposal_then_counter_questions_freeze_modified_damage_once() {
        let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        let catalog = CardDef::catalog(&faces);
        let mut defs=vec![CardDef::from_faces(catalog.get("Furnace of Rath").unwrap()).unwrap(),CardDef::from_faces(catalog.get("Torbran, Thane of Red Fell").unwrap()).unwrap(),
            CardDef::new("Red","{R}","Creature","Infect\nToxic 2\nLifelink",Some((3,30))).unwrap(),
            CardDef::new("Plus","{0}","Enchantment","If one or more energy counters would be put on a player, that many plus one energy counters are put on that player instead.",None).unwrap(),
            CardDef::new("Double","{0}","Enchantment","If one or more energy counters would be put on a player, twice that many energy counters are put on that player instead.",None).unwrap()];
        for d in &defs {
            crate::play::check(d).unwrap();
        }
        // Typed result fixtures only: poison replacement text is not an admission claim.
        for d in &mut defs[3..] {
            let crate::replace::Replacement::Counters { kinds: crate::replace::CounterScope::Named(kind), .. } =
                d.replacements[0].result.as_mut().unwrap()
            else {
                panic!("fixture")
            };
            *kind = "poison".into();
        }
        let mut g = Game::new(defs, 3, 20);
        let furnace = g.put(0, 0, Zone::Battlefield);
        let torbran = g.put(0, 1, Zone::Battlefield);
        let source = g.put(0, 2, Zone::Battlefield);
        let plus = g.put(0, 3, Zone::Battlefield);
        let double = g.put(0, 4, Zone::Battlefield);
        let start = g.log.len();
        g.damage_as_rule(&[(source, Target::Player(1), 3)], true).unwrap();
        let mut changed = g.clone();
        choose(&mut g, 1, furnace, source, 0);
        choose(&mut changed, 1, torbran, source, 0);
        for (mut branch, damage) in [(g, 8), (changed, 10)] {
            assert_eq!(branch.log.len(), start);
            assert_eq!(branch.players[0].life, 20);
            assert_eq!(branch.players[1].life, 20);
            assert!(branch.counters(Target::Player(1)).is_empty());
            let Some(Decision::Choose {
                player: 1,
                choice: Choice::Replacement { count, .. },
            }) = branch.decision.clone()
            else {
                panic!("{:?}", branch.decision)
            };
            assert_eq!(
                count,
                damage + 2,
                "Toxic stays fixed; Infect reads modified damage"
            );
            let mut counter_copy = branch.clone();
            for (game, first, poison) in [
                (&mut branch, plus, (damage + 3) * 2),
                (&mut counter_copy, double, (damage + 2) * 2 + 1),
            ] {
                let Some(Decision::Choose {
                    player,
                    choice: Choice::Replacement { among, .. },
                }) = game.decision.clone()
                else {
                    panic!("result choice")
                };
                assert_eq!(player, 1);
                let effect = ReplacementId::from_ability(first, crate::binding::AbilityKey::printed(0), 0);
                assert!(among.contains(&effect));
                game.act(player, Move::Choose(Answer::Replacement(effect)))
                    .unwrap();
                assert!(game.players[1].lost);
                assert_eq!(game.players[0].life, 20 + damage as i32);
                assert_eq!(
                    game.log
                        .iter()
                        .skip(start)
                        .filter(|e| matches!(e,Event::DamageDealt{amount,..} if *amount==damage))
                        .count(),
                    1
                );
                assert_eq!(game.log.iter().skip(start).filter(|e|matches!(e,Event::CountersPut{on:Target::Player(1),count,..} if *count==poison)).count(),1);
            }
        }
    }
}

#[cfg(test)]
mod counter_inventory_grouping {
    use crate::{CardDef,Event,Game,Target,Zone};
    /// CR120.4/700.1: simultaneous counter results share the holder/placing-actor
    /// event, not the damage-source lifelink partition (119.9). Two different
    /// controllers remain distinct actors for Doc's "you would put" (122.6).
    #[test]
    fn r120_4_simultaneous_sources_share_only_the_same_actor_holder_inventory() {
        let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        let catalog=CardDef::catalog(&faces);
        let doc=CardDef::from_faces(&catalog["Doc Samson, Super Psychiatrist"]).unwrap();
        for different_actor in [false,true] {
            let mut game=Game::new(vec![doc.clone(),
                CardDef::new("Infector","{0}","Creature","Infect",Some((2,2))).unwrap(),
                CardDef::new("Durable","{0}","Creature","",Some((10,10))).unwrap()],2,20);
            game.put(0,0,Zone::Battlefield);let a=game.put(0,1,Zone::Battlefield);
            let b=game.put(usize::from(different_actor),1,Zone::Battlefield);let target=game.put(0,2,Zone::Battlefield);
            let start=game.log.len();game.damage_as_rule(&[(a,target.into(),1),(b,target.into(),2)],false).unwrap();
            assert_eq!(game.obj(target).counters.get("-1/-1"),Some(&4));
            let actual:Vec<_>=game.log[start..].iter().filter_map(|e|if let Event::CountersPut{on:Target::Object(on),kind,count,..}=e {
                (*on==target && kind=="-1/-1").then_some(*count)
            }else{None}).collect();
            assert_eq!(actual,if different_actor{vec![2,2]}else{vec![4]});
            assert_eq!(game.log[start..].iter().filter(|e|matches!(e,Event::DamageDealt{..})).count(),2);
        }
    }
}

#[cfg(test)]
mod excess_occurrences {
    use crate::{CardDef,Game,Move,Zone};
    fn native(name:&str)->CardDef {
        let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        let d=CardDef::from_faces(&CardDef::catalog(&faces)[name]).unwrap();crate::play::check(&d).unwrap();d
    }
    fn creature(name:&str,p:i32,t:i32,text:&str)->CardDef {CardDef::new(name,"{0}","Creature",text,Some((p,t))).unwrap()}
    fn passes(g:&mut Game){for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}}
    fn treasures(g:&Game)->usize {g.battlefield.iter().filter(|o|g.obj(**o).chars.name=="Treasure Token").count()}
#[test]
fn r603_2c_native_galleon_groups_multiple_recipients_but_not_separate_occurrences() {
    let mut g=Game::new(vec![native("Magmatic Galleon"),creature("Source",1,1,""),creature("Wall",1,5,"Indestructible")],2,20);
    g.put(0,0,Zone::Battlefield);let source=g.put(0,1,Zone::Battlefield);let peer=g.put(0,1,Zone::Battlefield);
    let a=g.put(1,2,Zone::Battlefield);let b=g.put(1,2,Zone::Battlefield);
    // Neither three-point hit individually exceeds five toughness. Together they do.
    g.damage_as_rule(&[(source,a.into(),3),(peer,a.into(),3),(source,b.into(),7)],false).unwrap();g.settle().unwrap();
    assert_eq!(g.stack.len(),1);passes(&mut g);
    assert_eq!(treasures(&g),1);
    g.damage_as_rule(&[(source,a.into(),1)],false).unwrap();g.settle().unwrap();passes(&mut g);
    assert_eq!(treasures(&g),2);
}
#[test]
fn r120_10_native_galleon_excludes_combat_equal_lethal_and_wrong_controller() {
    for (combat,count,controller,expected)in [(true,3,1,0),(false,2,1,0),(false,3,0,0),(false,3,1,1)] {
        let mut g=Game::new(vec![native("Magmatic Galleon"),creature("Source",1,1,""),creature("Wall",1,2,"Indestructible")],2,20);
        g.put(0,0,Zone::Battlefield);let source=g.put(0,1,Zone::Battlefield);let wall=g.put(controller,2,Zone::Battlefield);
        g.damage_as_rule(&[(source,wall.into(),count)],combat).unwrap();g.settle().unwrap();
        if !g.stack.is_empty(){passes(&mut g);}
        assert_eq!(treasures(&g),expected);
    }
}
#[test]
fn r603_2_ordinary_damage_watcher_does_not_match_derived_excess_fact() {
    let mut g=Game::new(vec![creature("Source",1,1,""),creature("Wall",1,2,"Indestructible"),
        creature("Watcher",1,1,"Whenever a creature is dealt damage, you gain 1 life.")],2,20);
    let source=g.put(0,0,Zone::Battlefield);let wall=g.put(1,1,Zone::Battlefield);g.put(0,2,Zone::Battlefield);
    g.damage_as_rule(&[(source,wall.into(),3)],false).unwrap();g.settle().unwrap();
    assert_eq!(g.stack.len(),1);passes(&mut g);assert_eq!(g.players[0].life,21);
}
#[test]
fn r120_10_fully_prevented_damage_has_no_derived_excess_occurrence() {
    let mut g=Game::new(vec![native("Magmatic Galleon"),creature("Source",1,1,"Deathtouch"),creature("Wall",1,2,"Prevent all damage that would be dealt to this creature.")],2,20);
    g.put(0,0,Zone::Battlefield);let source=g.put(0,1,Zone::Battlefield);let wall=g.put(1,2,Zone::Battlefield);
    let start=g.log.len();
    g.damage_as_rule(&[(source,wall.into(),9)],false).unwrap();g.settle().unwrap();
    assert!(g.stack.is_empty());assert_eq!(g.obj(wall).damage,0);
    assert!(!g.log[start..].iter().any(|e|matches!(e,super::Event::ExcessDamageDealt{..})));
}

}
