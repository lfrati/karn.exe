//! A revealed-this-way predicate reads the reveal, not a preceding private look.
use crate::support::{cite, oracle};
use game::{Game, Move, Zone};
use game::state::{Answer, Decision};

#[test]
fn r701_27_delver_transforms_only_for_a_revealed_instant_or_sorcery() {
    for (top, reveal, transforms) in [("Lightning Bolt", true, true), ("Island", true, false),
        ("Lightning Bolt", false, false)] {
        let mut g = Game::new(vec![oracle("Delver of Secrets // Insectile Aberration"), oracle(top)], 2, 20);
        let delver = g.put(0, 0, Zone::Battlefield);
        g.put(0, 1, Zone::Library);
        g.part = ir::Part::Upkeep;
        g.log.push(game::Event::Began { part: ir::Part::Upkeep });
        // Let the driver collect the step trigger, then resolve to its explicit offer.
        for _ in 0..4 {
            if matches!(g.decision, Some(Decision::Choose { .. })) { break; }
            g.act(g.priority, Move::Pass).unwrap();
        }
        let mut fork = g.clone();
        for g in [&mut g, &mut fork] {
            cite!(["608.2d"]; ok: g.act(0, Move::Choose(if reveal { Answer::Yes } else { Answer::No })));
            cite!(["701.27a", "608.2h"]; assert_eq!(g.obj(delver).transformed, transforms));
            cite!(["701.20a"]; assert_eq!(g.players[0].library.len(), 1));
            cite!(["701.27a"]; assert_eq!(g.power(delver), Some(if transforms { 3 } else { 1 })));
        }
    }
}
