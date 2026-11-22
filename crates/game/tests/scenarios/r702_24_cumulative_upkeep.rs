//! Generic cumulative upkeep uses one atomic payment after the age counter is placed.
use crate::support::{cite, mana_pool, oracle, oracle_read};
use game::state::{Answer, Choice, Decision};
use game::{Game, Move, ObjId, Zone};
use ir::Part;

fn fixture() -> (Game, ObjId) {
    let mut g = Game::new(vec![oracle("Mystic Remora"), oracle("Island"), oracle("Solemnity")], 2, 20);
    for p in 0..2 { for _ in 0..20 { g.put(p, 1, Zone::Library); } }
    let remora = g.put(0, 0, Zone::Battlefield);
    (g, remora)
}
fn next_upkeep(g: &mut Game) {
    let turn = g.turn;
    for _ in 0..200 {
        if g.turn > turn && g.active == 0 && g.part == Part::Upkeep { return; }
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            None => g.act(g.priority, Move::Pass).unwrap(),
            other => panic!("unexpected question {other:?}"),
        }
    }
    panic!("no upkeep");
}
fn resolve(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn offered(g: &Game) -> u32 {
    let Some(Decision::Choose { player: 0, choice: Choice::Pay { cost, .. } }) = &g.decision else { panic!("{:?}", g.decision) };
    let [ir::mana::ManaSymbol::Generic(n)] = cost.0.as_slice() else { panic!("{cost:?}") };
    *n
}
#[test]
fn r702_24a_age_accumulates_and_partial_payment_does_not_count() {
    let (mut g, remora) = fixture();
    next_upkeep(&mut g); resolve(&mut g);
    cite!(["702.24a"]; assert_eq!((g.obj(remora).counters.get("age"), offered(&g)), (Some(&1), 1)));
    g.players[0].pool = mana_pool("C");
    g.act(0, Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool("C")), tapped: vec![] })).unwrap();
    cite!(["702.24a"]; assert!(g.battlefield.contains(&remora)));
    next_upkeep(&mut g); resolve(&mut g);
    cite!(["702.24a"]; assert_eq!((g.obj(remora).counters.get("age"), offered(&g)), (Some(&2), 2)));
    g.players[0].pool = mana_pool("CC");
    cite!(["702.24a", "118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool("C")), tapped: vec![] })));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["702.24a"]; assert!(!g.battlefield.contains(&remora)));
}
#[test]
fn r702_24a_no_age_counters_allows_zero_payment_and_departed_source_has_no_payment() {
    let (mut g, remora) = fixture();
    g.put(0, 2, Zone::Battlefield);
    next_upkeep(&mut g); resolve(&mut g);
    cite!(["702.24a", "101.2"]; assert_eq!(offered(&g), 0));
    g.act(0, Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool("")), tapped: vec![] })).unwrap();
    cite!(["702.24a"]; assert!(g.battlefield.contains(&remora)));
    let (mut g, remora) = fixture();
    next_upkeep(&mut g);
    g.destroy(remora);
    resolve(&mut g);
    cite!(["603.4", "702.24a"]; assert!(g.decision.is_none()));
}
#[test]
fn repeated_nongeneric_and_choice_costs_remain_refused() {
    // Uncited (admission): the shared atomic multiplier supports fixed generic mana only.
    crate::support::refused(game::play::check(&oracle_read("Uktabi Efreet")).map_err(|e| e.0), "keyword: Cumulative Upkeep");
}
