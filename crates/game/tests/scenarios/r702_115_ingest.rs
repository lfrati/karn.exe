//! Ingest retains the damaged player and exiles one public top card for each trigger, not each damage point.
use crate::support::{cite, oracle};
use game::state::Decision;
use game::{Game, Move, ObjId, Target, Zone};
fn resolve(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn game(empty: bool) -> (Game, ObjId) {
    let mut g = Game::new(
        ["Mist Intruder", "Forest", "Island", "Mountain", "Swamp"].into_iter().map(oracle).collect::<Vec<_>>(),
        3,
        20,
    );
    for _ in 0..2 {
        g.put(0, 1, Zone::Library);
        g.put(2, 4, Zone::Library);
    }
    if !empty {
        g.put(1, 2, Zone::Library);
        g.put(1, 3, Zone::Library);
    }
    let source = g.put(0, 0, Zone::Battlefield);
    g.put_counters(source, "+1/+1", 1);
    (g, source)
}
fn hit(g: &mut Game, source: ObjId) {
    for _ in 0..50 {
        if matches!(g.decision, Some(Decision::Attackers { player: 0 })) {
            break;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(0, Move::Attack(vec![(source, Target::Player(1))])).unwrap();
    for _ in 0..50 {
        if g.players[1].life == 18 {
            return;
        }
        match g.decision.clone() {
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            None => g.act(g.priority, Move::Pass).unwrap(),
            other => panic!("unexpected question {other:?}"),
        }
    }
    panic!("no combat damage");
}
#[test]
fn r702_115a_two_damage_exiles_one_card_from_only_the_damaged_players_library_after_source_departure() {
    for empty in [false, true] {
        let (mut g, source) = game(empty);
        hit(&mut g, source);
        cite!(["702.115a"];assert_eq!(g.stack.len(),1));
        assert!(g.exile.is_empty());
        g.destroy(source);
        resolve(&mut g);
        cite!(["702.115a","113.7a","609.3"];assert_eq!(g.exile.len(),usize::from(!empty)));
        assert_eq!(g.players[0].library.len(), 2);
        assert_eq!(g.players[2].library.len(), 2);
        assert_eq!(g.players[1].library.len(), usize::from(!empty));
        if !empty {
            let card = g.exile[0];
            assert_eq!(g.obj(card).chars.name, "Mountain");
            assert_eq!(g.obj(card).owner, 1);
            cite!(["406.3"];assert!((0..3).all(|p|g.sees(p,card))));
        }
    }
}
#[test]
fn r702_115a_noncombat_damage_does_not_ingest() {
    let (mut g, source) = game(false);
    g.damage(source, Target::Player(1), 1);
    g.act(g.priority, Move::Pass).unwrap();
    cite!(["702.115a"];assert!(g.stack.is_empty()&&g.exile.is_empty()));
    assert_eq!(g.players[1].library.len(), 2);
}
