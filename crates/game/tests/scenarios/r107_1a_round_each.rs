//! A trailing rounding instruction binds both halves, and a cast trigger keeps its spell's X.
use crate::support::{cite, mana_pool, oracle};
use game::{Game, Move, Target, Zone};

fn pass(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }

#[test]
fn r107_1a_hydroid_rounds_each_quantity_down_and_keeps_announced_x_after_countering() {
    for x in [0,1,2,3,5] {
        for countered in [false,true] {
            let mut g = Game::new(vec![oracle("Hydroid Krasis"), oracle("Forest"), oracle("Counterspell")], 2, 20);
            for _ in 0..5 { g.put(0, 1, Zone::Library); }
            let card = g.put(0, 0, Zone::Hand);
            g.players[0].pool = mana_pool("GU");
            g.players[0].pool.colorless = x;
            let payment = g.players[0].pool.clone();
            g.act(0, Move::Paying(payment, Box::new(Move::WithX(x, Box::new(Move::Cast { card, targets: vec![] }))))).unwrap();
            cite!(["603.2", "603.3"]; assert_eq!(g.stack.len(), 2));
            if countered {
                let spell = g.stack[0];
                g.act(0, Move::Pass).unwrap();
                let counter = g.put(1, 2, Zone::Hand);
                g.players[1].pool = mana_pool("UU");
                g.act(1, crate::support::paid("UU", Move::Cast { card: counter, targets: vec![vec![Target::Object(spell)]] })).unwrap();
                pass(&mut g);
            }
            pass(&mut g); // The cast trigger resolves independently of its spell.
            cite!(["107.1a", "113.7a"]; case:format!("X={x},countered={countered}"); assert_eq!(g.players[0].life,20+(x/2) as i32));
            cite!(["107.1a", "113.7a"]; assert_eq!(g.players[0].hand.len(),(x/2) as usize));
            if !countered { pass(&mut g); }
            let hydroid = g.battlefield.iter().find(|o| g.obj(**o).card == Some(0));
            if x > 0 && !countered {
                let hydroid = *hydroid.expect("the spell entered with X counters");
                cite!(["122.1a"]; assert_eq!(g.power(hydroid),Some(x as i32)));
            } else { cite!(["701.5a", "704.5f"]; assert!(hydroid.is_none())); }
            cite!(["608.2c"]; assert!(g.stack.is_empty() && g.decision.is_none()));
        }
    }
}
