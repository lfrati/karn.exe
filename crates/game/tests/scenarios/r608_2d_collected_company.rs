//! Bounded choices select from the retained looked-at group, then order its remainder.
use crate::support::{cite, oracle, mana_pool, paid};
use game::{Game, Move, Zone};
use game::state::{Answer, Choice, Decision};

#[test]
fn r608_2d_collected_company_selects_zero_one_or_two_small_creatures() {
    for count in 0..=2 {
        let defs: Vec<_> = ["Collected Company", "Grizzly Bears", "Llanowar Elves", "Hill Giant", "Island", "Lightning Bolt"]
            .into_iter().map(oracle).collect();
        let mut g = Game::new(defs, 2, 20);
        let hidden = g.put(0, 1, Zone::Library);
        let bear = g.put(0, 1, Zone::Library);
        let elf = g.put(0, 2, Zone::Library);
        let giant = g.put(0, 3, Zone::Library);
        let land = g.put(0, 4, Zone::Library);
        let bolt = g.put(0, 5, Zone::Library);
        let other = g.put(0, 1, Zone::Library);
        let spell = g.put(0, 0, Zone::Hand);
        g.players[0].pool = mana_pool("CCCG");
        g.act(0, paid("CCCG", Move::Cast { card: spell, targets: vec![] })).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        let Some(Decision::Choose { player: 0, choice: Choice::Select { min, max, among } }) = &g.decision else { panic!("{:?}", g.decision) };
        cite!(["608.2d"]; assert_eq!((*min, *max), (0, 2)));
        cite!(["608.2d"]; assert_eq!(among, &vec![other, elf, bear]));
        cite!(["608.2d"]; refused("3 chosen (608.2d): choose between 0 and 2 distinct matching objects"): g.act(0, Move::Choose(Answer::Objects(vec![other, elf, bear]))));
        cite!(["608.2d"]; refused("a chosen object is not among the offered objects (608.2d)"): g.act(0, Move::Choose(Answer::Objects(vec![giant]))));
        let chosen = vec![other, elf][..count].to_vec();
        let mut fork = g.clone();
        for g in [&mut g, &mut fork] {
            g.act(0, Move::Choose(Answer::Objects(chosen.clone()))).unwrap();
            let remainder: Vec<_> = [other, bolt, land, giant, elf, bear].into_iter().filter(|o| !chosen.contains(o)).collect();
            cite!(["608.2c"]; assert_eq!(g.decision, Some(Decision::Choose { player: 0,
                choice: Choice::OrderCards { to: game::state::Placement::Library(ir::action::LibraryPosition::Bottom), cards: remainder.clone() } })));
            g.act(0, Move::Choose(Answer::Objects(remainder.clone()))).unwrap();
            cite!(["608.2c"]; assert_eq!(g.battlefield.len(), count));
            cite!(["608.2c"]; assert!(g.players[0].library.contains(&hidden)));
            cite!(["608.2c"]; assert_eq!(g.players[0].library.len(), 7 - count));
            cite!(["608.2c"]; assert!(g.stack.is_empty() && g.decision.is_none()));
        }
    }
}
