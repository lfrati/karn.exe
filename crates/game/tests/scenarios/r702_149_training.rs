//! Training checks another greater-power attacker only when the attack event occurs.
use crate::support::{cite, mana_pool, oracle, oracle_read, paid};
use game::state::Decision;
use game::{Game, Move, ObjId, Target, Zone};
fn resolve(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn attack(g: &mut Game, attacks: Vec<(ObjId, Target)>) {
    for _ in 0..40 {
        if matches!(g.decision, Some(Decision::Attackers { player: 0 })) {
            g.act(0, Move::Attack(attacks)).unwrap();
            return;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("no attackers");
}
#[test]
fn r702_149a_another_defender_is_allowed_and_the_larger_attacker_need_not_survive_resolution() {
    let mut g = Game::new(vec![oracle("Gryff Rider"), oracle("Hill Giant"), oracle("Lightning Bolt")], 3, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let larger = g.put(0, 1, Zone::Battlefield);
    attack(&mut g, vec![(source, Target::Player(1)), (larger, Target::Player(2))]);
    cite!(["702.149a"];assert_eq!(g.stack.len(),1));
    let bolt = g.put(0, 2, Zone::Hand);
    g.players[0].pool = mana_pool("R");
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![larger.into()]] })).unwrap();
    resolve(&mut g);
    assert!(!g.battlefield.contains(&larger));
    resolve(&mut g);
    cite!(["702.149a","603.2"];assert_eq!(g.counters(source).get("+1/+1"),Some(&1)));
}
#[test]
fn r702_149a_equal_power_or_a_larger_creature_staying_home_does_not_train() {
    let mut g = Game::new(vec![oracle("Gryff Rider"), oracle("Grizzly Bears"), oracle("Hill Giant")], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let equal = g.put(0, 1, Zone::Battlefield);
    g.put(0, 2, Zone::Battlefield);
    attack(&mut g, vec![(source, Target::Player(1)), (equal, Target::Player(1))]);
    cite!(["702.149a"];assert!(g.stack.is_empty()&&g.counters(source).is_empty()));
}
#[test]
fn training_event_consumers_stay_refused_until_successful_training_emits_its_own_receipt() {
    // Uncited (admission): a general counter placement is not a training event (702.149c).
    crate::support::refused(game::play::check(&oracle_read("Savior of Ollenbock")).map_err(|e| e.0), "trigger: Train");
}

#[test]
fn r122_1b_senseis_named_training_counter_does_not_grant_the_training_keyword() {
    let mut g = Game::new(vec![oracle("Sensei Golden-Tail"), oracle("Grizzly Bears"), oracle("Hill Giant")], 2, 20);
    let sensei = g.put(0, 0, Zone::Battlefield);
    let receiver = g.put(0, 1, Zone::Battlefield);
    let larger = g.put(0, 2, Zone::Battlefield);
    g.players[0].pool = mana_pool("WW");
    g.act(
        0,
        paid("WW", Move::Activate { source: sensei, ability: 1, targets: vec![vec![receiver.into()]], paying: vec![] }),
    )
    .unwrap();
    resolve(&mut g);
    cite!(["122.1b"]; assert_eq!(g.counters(receiver).get("training"), Some(&1)));
    cite!(["122.1b"]; assert!(!g.has(receiver, "Training")));
    // Uncited (Oracle): Sensei explicitly grants bushido; its named counter grants no ability.
    assert!(g.has(receiver, "Bushido"));
    g.put_counters(receiver, "flying", 1);
    cite!(["122.1b"]; assert!(g.has(receiver, "Flying") && !g.has(receiver, "Training")));
    attack(&mut g, vec![(receiver, Target::Player(1)), (larger, Target::Player(1))]);
    cite!(["122.1b"]; assert!(g.stack.is_empty()));
}
