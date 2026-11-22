//! Bloodthirst is an entry replacement, counting damage rather than life loss (702.54).
use crate::support::{cite, mana_pool, oracle, paid};
use game::{Game, Move, Target, Zone};

fn fixture() -> Game {
    Game::new(vec![oracle("Bloodscale Prowler"), oracle("Grizzly Bears"), oracle("Hardened Scales")], 3, 20)
}
fn enter(g: &mut Game) -> game::ObjId {
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool = mana_pool("RRR");
    g.act(0, paid("RRR", Move::Cast { card, targets: vec![] })).unwrap();
    for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); }
    *g.battlefield.iter().find(|o| g.obj(**o).card == Some(0)).unwrap()
}
#[test]
fn r702_54a_only_opponent_damage_enables_bloodthirst_in_multiplayer() {
    for damaged in [0, 1, 2] {
        let mut g = fixture();
        let source = g.put(1, 1, Zone::Battlefield);
        g.damage(source, Target::Player(damaged), 1);
        let prowler = enter(&mut g);
        cite!(["702.54a"]; case: damaged; assert_eq!(g.obj(prowler).counters.get("+1/+1").copied().unwrap_or(0), u32::from(damaged != 0)));
    }
}
#[test]
fn r702_54a_undamaged_opponent_and_damage_to_a_creature_do_not_qualify() {
    let mut g = fixture();
    let source = g.put(1, 1, Zone::Battlefield);
    let creature = g.put(2, 1, Zone::Battlefield);
    g.damage(source, Target::Object(creature), 1);
    let prowler = enter(&mut g);
    cite!(["702.54a"]; assert!(g.obj(prowler).counters.is_empty()));
}
#[test]
fn r702_54a_bloodthirst_counters_pass_through_counter_replacements() {
    let mut g = fixture();
    g.put(0, 2, Zone::Battlefield);
    let source = g.put(1, 1, Zone::Battlefield);
    g.damage(source, Target::Player(1), 1);
    let prowler = enter(&mut g);
    cite!(["702.54a", "614.1a"]; assert_eq!(g.obj(prowler).counters.get("+1/+1"), Some(&2)));
}
