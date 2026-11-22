//! Dethrone matches the actual attacked player at attack time and each instance triggers separately.
use crate::support::{cite, mana_pool, oracle, paid};
use game::{Game, Move, ObjId, Target, Zone};
use game::state::Decision;
fn resolve(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn attackers(g: &mut Game, attacks: Vec<(ObjId, Target)>) {
    for _ in 0..40 {
        if matches!(g.decision, Some(Decision::Attackers { player: 0 })) {
            g.act(0, Move::Attack(attacks)).unwrap(); return;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("no attack declaration");
}
#[test]
fn r702_105a_most_life_includes_ties_and_is_not_rechecked_after_a_response() {
    let mut g = Game::new(vec![oracle("Grenzo's Cutthroat"), oracle("Lightning Bolt")], 3, 20);
    g.players[2].life = 19;
    let high = g.put(0, 0, Zone::Battlefield);
    let low = g.put(0, 0, Zone::Battlefield);
    attackers(&mut g, vec![(high, Target::Player(1)), (low, Target::Player(2))]);
    cite!(["702.105a"]; assert_eq!(g.stack.len(), 1));
    let bolt = g.put(0, 1, Zone::Hand); g.players[0].pool = mana_pool("R");
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] })).unwrap();
    resolve(&mut g); assert_eq!(g.players[1].life, 17);
    resolve(&mut g);
    cite!(["702.105a", "603.2"]; assert_eq!(g.counters(high).get("+1/+1"), Some(&1)));
    assert!(g.counters(low).is_empty());
}
#[test]
fn r702_105a_attacking_a_planeswalker_is_not_attacking_its_controller() {
    let mut g = Game::new(vec![oracle("Grenzo's Cutthroat"), oracle("Jace Beleren")], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let walker = g.put(1, 1, Zone::Battlefield); g.put_counters(walker, "loyalty", 3);
    attackers(&mut g, vec![(source, Target::Object(walker))]);
    cite!(["702.105a", "508.1b"]; assert!(g.stack.is_empty() && g.counters(source).is_empty()));
}
#[test]
fn r702_105b_marchesa_grants_a_second_independent_instance() {
    let mut g = Game::new(vec![oracle("Grenzo's Cutthroat"), oracle("Marchesa, the Black Rose")], 2, 20);
    g.put(0, 1, Zone::Battlefield);
    let source = g.put(0, 0, Zone::Battlefield);
    attackers(&mut g, vec![(source, Target::Player(1))]);
    let Some(Decision::Order { player: 0, abilities }) = &g.decision else { panic!("{:?}",g.decision) };
    cite!(["702.105b", "613.1f"]; assert_eq!(abilities.len(), 2));
    g.act(0, Move::Order(vec![0,1])).unwrap(); resolve(&mut g); resolve(&mut g);
    cite!(["702.105b"]; assert_eq!(g.counters(source).get("+1/+1"), Some(&2)));
}
