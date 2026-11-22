//! Affinity uses ordinary self cost reductions and effective characteristics (702.41).
use crate::support::{cite, mana_pool, oracle, paid};
use game::{Game, Move, Zone};

#[test]
fn r702_41a_affinity_counts_only_your_artifacts_and_keeps_exact_payment() {
    let mut g = Game::new(vec![oracle("Myr Enforcer"), oracle("Memnite")], 2, 20);
    for _ in 0..3 { g.put(0, 1, Zone::Battlefield); }
    for _ in 0..7 { g.put(1, 1, Zone::Battlefield); }
    let enforcer = g.put(0, 0, Zone::Hand);
    g.players[0].pool = mana_pool("CCCC");
    cite!(["702.41a", "118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, paid("CCC", Move::Cast { card: enforcer, targets: vec![] })));
    cite!(["702.41a", "601.2f"]; ok: g.act(0, paid("CCCC", Move::Cast { card: enforcer, targets: vec![] })));
}
#[test]
fn r702_41a_affinity_cannot_reduce_the_colored_part_of_thoughtcast() {
    let mut g = Game::new(vec![oracle("Thoughtcast"), oracle("Memnite"), oracle("Island")], 2, 20);
    for _ in 0..5 { g.put(0, 1, Zone::Battlefield); }
    for _ in 0..3 { g.put(0, 2, Zone::Library); }
    let spell = g.put(0, 0, Zone::Hand);
    g.players[0].pool = mana_pool("U");
    cite!(["702.41a", "118.7a"]; ok: g.act(0, paid("U", Move::Cast { card: spell, targets: vec![] })));
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["608.2c"]; assert_eq!(g.players[0].hand.len(), 2));
}
#[test]
fn r702_41a_the_parameter_selects_forests_rather_than_every_land() {
    let mut g = Game::new(vec![oracle("Tangle Golem"), oracle("Forest"), oracle("Island")], 2, 20);
    for _ in 0..2 { g.put(0, 1, Zone::Battlefield); }
    for _ in 0..4 { g.put(0, 2, Zone::Battlefield); }
    let spell = g.put(0, 0, Zone::Hand);
    g.players[0].pool = mana_pool("CCCCC");
    cite!(["702.41a"]; ok: g.act(0, paid("CCCCC", Move::Cast { card: spell, targets: vec![] })));
}
