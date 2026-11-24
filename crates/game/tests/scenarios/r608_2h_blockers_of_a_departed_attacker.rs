//! "Sacrifice it. If you do, it deals 4 damage to each creature blocking it" (608.2h): the
//! creatures blocking it are read as the attacker last existed on the battlefield, though
//! leaving removed it from combat (506.4).
use crate::support::{cite, oracle};
use game::state::Decision;
use game::{Game, Move, ObjId, Target, Zone};

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

fn block(g: &mut Game, blocks: Vec<(ObjId, ObjId)>) {
    for _ in 0..40 {
        if matches!(g.decision, Some(Decision::Blockers { player: 1 })) {
            g.act(1, Move::Block(blocks)).unwrap();
            return;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("no blockers");
}

#[test]
fn r608_2h_ib_halfheart_damages_each_creature_that_was_blocking_the_sacrificed_goblin() {
    // Ib Halfheart: "Whenever another Goblin you control becomes blocked, sacrifice it. If you
    // do, it deals 4 damage to each creature blocking it."
    let mut g = Game::new(vec![oracle("Ib Halfheart, Goblin Tactician"), oracle("Goblin Piker"), oracle("Grizzly Bears")], 2, 20);
    g.put(0, 0, Zone::Battlefield);
    let goblin = g.put(0, 1, Zone::Battlefield);
    let (a, b) = (g.put(1, 2, Zone::Battlefield), g.put(1, 2, Zone::Battlefield));
    let bystander = g.put(1, 2, Zone::Battlefield);
    attack(&mut g, vec![(goblin, Target::Player(1))]);
    block(&mut g, vec![(a, goblin), (b, goblin)]);
    cite!(["509.3c", "603.2"]; assert_eq!(g.stack.len(), 1, "Ib's trigger"));
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["701.21a"]; assert!(!g.battlefield.contains(&goblin), "the Goblin is sacrificed"));
    cite!(["608.2h", "120.3e"]; assert_eq!((g.obj(a).damage, g.obj(b).damage), (4, 4), "each creature that was blocking it is dealt 4"));
    cite!(["608.2h"]; assert_eq!(g.obj(bystander).damage, 0, "a creature that blocked nothing is not dealt damage"));
}
