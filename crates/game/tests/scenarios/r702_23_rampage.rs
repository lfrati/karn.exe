//! Rampage reads the current blocker count once on resolution, then keeps the resulting bonus.
use crate::support::{cite, mana_pool, oracle, paid};
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
fn resolve(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn blocked(count: usize) -> (Game, ObjId, Vec<ObjId>) {
    let mut g =
        Game::new(["Craw Giant", "Wall of Stone", "Unsummon"].into_iter().map(oracle).collect::<Vec<_>>(), 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let blockers = (0..count).map(|_| g.put(1, 1, Zone::Battlefield)).collect::<Vec<_>>();
    for _ in 0..40 {
        if matches!(g.decision, Some(Decision::Attackers { player: 0 })) {
            break;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(0, Move::Attack(vec![(source, Target::Player(1))])).unwrap();
    for _ in 0..40 {
        if matches!(g.decision, Some(Decision::Blockers { player: 1 })) {
            break;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(1, Move::Block(blockers.iter().map(|b| (*b, source)).collect())).unwrap();
    (g, source, blockers)
}
fn bounce(g: &mut Game, object: ObjId) {
    let card = g.put(0, 2, Zone::Hand);
    g.players[0].pool = mana_pool("U");
    g.act(0, paid("U", Move::Cast { card, targets: vec![vec![object.into()]] })).unwrap();
    resolve(g);
}
#[test]
fn r702_23b_counts_when_resolving_and_does_not_recalculate_after_the_bonus_is_created() {
    let (mut g, source, blockers) = blocked(3);
    cite!(["702.23a"];assert_eq!(g.stack.len(),1));
    bounce(&mut g, blockers[0]);
    resolve(&mut g);
    cite!(["702.23b"];assert_eq!((g.power(source),g.toughness(source)),(Some(8),Some(6))));
    bounce(&mut g, blockers[1]);
    cite!(["702.23b"];assert_eq!((g.power(source),g.toughness(source)),(Some(8),Some(6))));
}
#[test]
fn r702_23a_one_or_zero_remaining_blockers_still_has_the_trigger_but_no_bonus() {
    for remove in [false, true] {
        let (mut g, source, blockers) = blocked(1);
        cite!(["702.23a"];assert_eq!(g.stack.len(),1));
        if remove {
            bounce(&mut g, blockers[0]);
        }
        resolve(&mut g);
        cite!(["702.23a","107.1b"];assert_eq!((g.power(source),g.toughness(source)),(Some(6),Some(4))));
    }
}
#[test]
fn r107_1_literal_multiples_of_modeled_counts_use_checked_arithmetic() {
    let def = CardDef::new(
        "Twice the count",
        "{0}",
        "Sorcery",
        "You gain life equal to twice the number of creatures you control.",
        None,
    )
    .unwrap();
    game::play::check(&def).unwrap();
    for count in [0, 2] {
        let mut g = Game::new(vec![def.clone(), oracle("Grizzly Bears")], 2, 20);
        for _ in 0..count {
            g.put(0, 1, Zone::Battlefield);
        }
        g.put(1, 1, Zone::Battlefield);
        let card = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
        resolve(&mut g);
        cite!(["107.1","119.3"];assert_eq!(g.players[0].life,20+2*count));
    }
}
