//! The search/reveal producer and the explicit chosen-card result survive a targeted other chooser.
use crate::support::{cite, mana_pool, oracle, paid};
use game::{Game, Move, Target, Zone};
use game::state::{Answer, Choice, Decision};

#[test]
fn r608_2d_intuition_searches_three_but_the_targeted_opponent_chooses_the_hand_card() {
    let mut g = Game::new(["Intuition", "Forest", "Lightning Bolt", "Island", "Plains"].into_iter().map(oracle).collect::<Vec<_>>(), 3, 20);
    let forest = g.put(0, 1, Zone::Library);
    let bolt = g.put(0, 2, Zone::Library);
    let island = g.put(0, 3, Zone::Library);
    let hidden = g.put(0, 4, Zone::Library);
    let spell = g.put(0, 0, Zone::Hand);
    g.players[0].pool = mana_pool("UUU");
    g.act(0, paid("UUU", Move::Cast { card: spell, targets: vec![vec![Target::Player(1)]] })).unwrap();
    for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); }
    let Some(Decision::Choose { player: 0, choice: Choice::Search { among, min: 3, max: 3, .. } }) = &g.decision else { panic!("{:?}",g.decision) };
    cite!(["701.23a"]; assert_eq!(among.len(), 4));
    cite!(["701.23d"]; refused("choose between 3 and 3"): g.act(0, Move::Choose(Answer::Objects(vec![]))));
    g.act(0, Move::Choose(Answer::Objects(vec![forest, bolt, island]))).unwrap();
    let Some(Decision::Choose { player: 1, choice: Choice::Objects { among, count: 1, .. } }) = &g.decision else { panic!("{:?}",g.decision) };
    cite!(["608.2d"]; assert_eq!(among, &vec![forest,bolt,island]));
    cite!(["608.2d"]; refused("not among the offered objects"): g.act(1, Move::Choose(Answer::Objects(vec![hidden]))));
    let mut fork = g.clone();
    for (g, chosen) in [(&mut g, forest), (&mut fork, bolt)] {
        g.act(1, Move::Choose(Answer::Objects(vec![chosen]))).unwrap();
        let rest: Vec<_> = [forest,bolt,island].into_iter().filter(|o| *o!=chosen).collect();
        let Some(Decision::Choose { player:0, choice:Choice::OrderCards { cards, .. } }) = &g.decision else { panic!("{:?}",g.decision) };
        cite!(["404.3"]; assert_eq!(cards, &rest));
        cite!(["608.2f"]; assert!(g.players[0].hand.is_empty()));
        g.act(0, Move::Choose(Answer::Objects(rest))).unwrap();
        cite!(["608.2c"]; assert_eq!(g.players[0].hand.len(),1));
        cite!(["608.2c"]; assert_eq!(g.obj(g.players[0].hand[0]).card, g.obj(chosen).card));
        cite!(["608.2c"]; assert_eq!(g.players[0].graveyard.len(),3));
        cite!(["608.2c"]; assert_eq!(g.players[0].library.len(),1));
        cite!(["608.2c"]; assert!(g.stack.is_empty() && g.decision.is_none()));
    }
}
