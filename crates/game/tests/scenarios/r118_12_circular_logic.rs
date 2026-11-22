//! A per-card unless payment is locked at resolution and remains an explicit payment.
use crate::support::{cite, oracle, mana_pool, paid};
use game::{Game, Move, Target, Zone};
use game::state::{Answer, Choice, Decision};

#[test]
fn r118_12_circular_logic_counts_its_controllers_graveyard_before_payment() {
    for count in [0, 3] {
        let mut g = Game::new(vec![oracle("Circular Logic"), oracle("Lightning Bolt"), oracle("Island")], 2, 20);
        for _ in 0..count { g.put(0, 2, Zone::Graveyard); }
        for _ in 0..5 { g.put(1, 2, Zone::Graveyard); }
        g.active = 1; g.priority = 1;
        let bolt = g.put(1, 1, Zone::Hand);
        g.players[1].pool = mana_pool("R");
        g.act(1, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Player(0)]] })).unwrap();
        let target = *g.stack.last().unwrap();
        g.act(1, Move::Pass).unwrap();
        let logic = g.put(0, 0, Zone::Hand);
        g.players[0].pool = mana_pool("CCU");
        g.act(0, paid("CCU", Move::Cast { card: logic, targets: vec![vec![Target::Object(target)]] })).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        let Some(Decision::Choose { player: 1, choice: Choice::Pay { cost, .. } }) = &g.decision else { panic!("{:?}", g.decision) };
        cite!(["118.12a", "608.2h"]; assert_eq!(cost.0, vec![ir::mana::ManaSymbol::Generic(count)]));
        let mut decline = g.clone();
        decline.act(1, Move::Choose(Answer::No)).unwrap();
        cite!(["701.6a"]; assert!(decline.stack.is_empty()));
        cite!(["701.6a"]; assert_eq!(decline.players[0].life, 20));
        let units = "C".repeat(count as usize);
        g.players[1].pool = mana_pool(&units);
        g.act(1, Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool(&units)), tapped: vec![] })).unwrap();
        cite!(["118.12a"]; assert_eq!(g.stack, vec![target]));
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        cite!(["118.12a"]; assert_eq!(g.players[0].life, 17));
    }
}
