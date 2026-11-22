//! Modular transfers the departed permanent's actual counters through an explicit target and may choice.
use crate::support::{cite, mana_pool, oracle, paid};
use game::{Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};

fn setup() -> Game {
    Game::new(["Arcbound Worker", "Arcbound Ravager", "Ornithopter", "Grizzly Bears", "Hardened Scales"]
        .into_iter().map(oracle).collect::<Vec<_>>(), 2, 20)
}
fn resolve(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn cast(g: &mut Game, index: usize, mana: &str) -> ObjId {
    let card = g.put(0, index, Zone::Hand);
    g.players[0].pool = mana_pool(mana);
    g.act(0, paid(mana, Move::Cast { card, targets: vec![] })).unwrap();
    resolve(g);
    *g.battlefield.iter().find(|o| g.obj(**o).card == Some(index)).unwrap()
}
fn target(g: &mut Game, object: ObjId) {
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(object)]])).unwrap();
}
fn transfer(g: &mut Game, accept: bool) {
    resolve(g);
    assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { .. }, .. })));
    g.act(0, Move::Choose(if accept { Answer::Yes } else { Answer::No })).unwrap();
}

#[test]
fn r702_43a_worker_entry_and_death_use_actual_counters_not_printed_n() {
    let mut g = setup();
    g.put(0, 4, Zone::Battlefield);
    let worker = cast(&mut g, 0, "C");
    cite!(["702.43a", "614.1c"]; assert_eq!(g.counters(worker).get("+1/+1"), Some(&2)));
    g.put_counters(worker, "+1/+1", 3);
    g.put_counters(worker, "charge", 9);
    assert_eq!(g.counters(worker).get("+1/+1"), Some(&5));
    let ravager = cast(&mut g, 1, "CC");
    let artifact = g.put(1, 2, Zone::Battlefield);
    let nonartifact = g.put(1, 3, Zone::Battlefield);
    g.act(0, Move::Activate { source: ravager, ability: 0, targets: vec![], paying: vec![vec![worker]] }).unwrap();
    cite!(["702.43a", "603.3d"]; assert!(matches!(g.decision, Some(Decision::Targets { .. }))));
    cite!(["702.43a", "601.2c"]; refused("an illegal target"): g.act(0, Move::ChooseTargets(vec![vec![nonartifact.into()]])));
    target(&mut g, artifact);
    let mut declined = g.clone();
    transfer(&mut declined, false);
    cite!(["702.43a"]; assert!(declined.counters(artifact).is_empty()));
    transfer(&mut g, true);
    cite!(["702.43a", "608.2h"]; assert_eq!(g.counters(artifact).get("+1/+1"), Some(&5)));
    assert_eq!(g.counters(artifact).len(), 1, "the nine charge counters are not transferred");
    assert_eq!(g.controller(artifact), 1, "modular permits another player's artifact creature");
}

#[test]
fn r704_7_modular_uses_counters_before_the_same_state_based_actions_kill_and_cancel() {
    let mut g = setup();
    g.put(0, 4, Zone::Battlefield);
    let worker = cast(&mut g, 0, "C");
    let artifact = g.put(1, 2, Zone::Battlefield);
    g.put_counters(worker, "-1/-1", 3);
    g.act(0, Move::Pass).unwrap();
    target(&mut g, artifact);
    transfer(&mut g, true);
    // Pinned Worker/Ravager ruling: LKI precedes the simultaneous cancellation
    // and death, so both original +1/+1 counters count even though the net is -1.
    cite!(["704.7", "702.43a"]; assert_eq!(g.counters(artifact).get("+1/+1"), Some(&2)));
}

#[test]
fn r702_43a_self_sacrificed_ravager_uses_lki_and_does_not_retarget_a_new_incarnation() {
    let mut g = setup();
    let ravager = cast(&mut g, 1, "CC");
    let artifact = g.put(1, 2, Zone::Battlefield);
    g.act(0, Move::Activate { source: ravager, ability: 0, targets: vec![], paying: vec![vec![ravager]] }).unwrap();
    target(&mut g, artifact);
    let mut valid = g.clone();
    transfer(&mut valid, true);
    cite!(["113.7a", "702.43a"]; assert_eq!(valid.counters(artifact).get("+1/+1"), Some(&1)));
    g.destroy(artifact);
    let later = g.put(1, 2, Zone::Battlefield);
    resolve(&mut g);
    cite!(["400.7", "608.2b"]; assert!(g.counters(later).is_empty() && g.decision.is_none()));
}
