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

/// Pass to the first attack declaration of this turn, declaring no attack or block before it.
fn attack_in_turn(g: &mut Game, turn: u32, attacks: Vec<(ObjId, Target)>) {
    for _ in 0..400 {
        match g.decision.clone() {
            Some(Decision::Attackers { player: 0 }) if g.turn == turn => { g.act(0, Move::Attack(attacks)).unwrap(); return; }
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("no attack in turn {turn}");
}

#[test]
fn r111_3_torens_token_with_training_trains() {
    // Torens: "Whenever you cast a creature spell, create a 1/1 green and white Human Soldier
    // creature token with training." The token has the triggered ability training is (702.149a).
    let mut g = Game::new(vec![oracle("Torens, Fist of the Angels"), oracle("Grizzly Bears"), oracle("Forest")], 2, 20);
    for p in 0..2 { for _ in 0..6 { g.put(p, 2, Zone::Library); } }
    let torens = g.put(0, 0, Zone::Battlefield);
    let bears = g.put(0, 1, Zone::Hand);
    g.players[0].pool = mana_pool("{1}{G}");
    g.act(0, paid("{1}{G}", Move::Cast { card: bears, targets: vec![] })).unwrap();
    resolve(&mut g);
    resolve(&mut g);
    let token = *g.battlefield.iter().find(|o| **o != torens && g.obj(**o).zone == Zone::Battlefield && g.obj(**o).card.is_none()).expect("the token");
    let bears = *g.battlefield.iter().find(|o| **o != torens && **o != token).expect("the Bears");
    let turn = g.turn + 2;
    attack_in_turn(&mut g, turn, vec![(token, Target::Player(1)), (bears, Target::Player(1))]);
    cite!(["702.149a", "111.3"]; assert_eq!(g.stack.len(), 1, "the token's training"));
    resolve(&mut g);
    cite!(["702.149a"]; assert_eq!(g.counters(token).get("+1/+1"), Some(&1)));
}

#[test]
fn r122_1b_an_exalted_counter_stays_refused_until_a_counter_gives_its_trigger() {
    // Uncited (admission): a keyword counter gives the keyword (122.1b); exalted is a triggered
    // ability (702.83a) whose instance the engine does not attach to a counter yet.
    crate::support::refused(game::play::check(&oracle_read("Emissary of Soulfire")).map_err(|e| e.0), "keyword counter: Exalted gives a triggered ability");
}
