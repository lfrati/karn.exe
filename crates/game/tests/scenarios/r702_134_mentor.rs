//! Mentor's target must attack and have strictly less power at announcement and resolution.
use crate::support::{cite, oracle};
use game::{Game, Move, ObjId, Target, Zone};
use game::state::Decision;
fn fixture() -> (Game, ObjId, ObjId, ObjId) {
    let mut g = Game::new(vec![oracle("Sunhome Stalwart"), oracle("Memnite"), oracle("Grizzly Bears")], 2, 20);
    g.set_ask_forced(true);
    let mentor = g.put(0, 0, Zone::Battlefield);
    let small = g.put(0, 1, Zone::Battlefield);
    let equal = g.put(0, 2, Zone::Battlefield);
    while g.decision != Some(Decision::Attackers { player: 0 }) { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(vec![(mentor, Target::Player(1)), (small, Target::Player(1)), (equal, Target::Player(1))])).unwrap();
    assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. })));
    (g, mentor, small, equal)
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
#[test]
fn r702_134a_target_requires_strictly_less_power_and_attacking_status() {
    let (mut g, mentor, small, equal) = fixture();
    let idle = g.put(0, 1, Zone::Battlefield);
    for invalid in [mentor, equal, idle] {
        cite!(["702.134a", "115.1d"]; refused("an illegal target"): g.act(0, Move::ChooseTargets(vec![vec![Target::Object(invalid)]])));
    }
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(small)]])).unwrap(); pass(&mut g);
    cite!(["702.134a"]; assert_eq!(g.obj(small).counters.get("+1/+1"), Some(&1)));
}
#[test]
fn r702_134a_matching_power_before_resolution_invalidates_the_mentor_target() {
    let (mut g, _, small, _) = fixture();
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(small)]])).unwrap();
    g.put_counters(small, "+1/+1", 1); pass(&mut g);
    cite!(["608.2b", "702.134a"]; assert_eq!(g.obj(small).counters.get("+1/+1"), Some(&1)));
    assert!(g.stack.is_empty());
}
#[test]
fn r702_134a_the_departed_mentor_uses_last_known_power() {
    let (mut g, mentor, small, _) = fixture();
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(small)]])).unwrap();
    g.destroy(mentor); pass(&mut g);
    cite!(["608.2h", "702.134a"]; assert_eq!(g.obj(small).counters.get("+1/+1"), Some(&1)));
}
#[test]
fn mentor_event_consumers_remain_explicitly_refused() {
    // Uncited (admission): a mentored-event producer is not part of this target/counter template.
    let def = game::CardDef::new("Unsupported mentor watcher", "{0}", "Creature", "Mentor\nWhenever this creature mentors a creature, you gain 1 life.", Some((2, 2))).unwrap();
    crate::support::refused(game::play::check(&def).map_err(|e| e.0), "trigger: Mentor");
}
