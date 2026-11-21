//! Player identity and opponent selection (102) share the same enumeration and legality.
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};

#[test]
fn r102_2_an_opponent_is_the_other_player_and_a_player_can_also_be_you() {
    for controller in 0..2 {
        let defs = vec![
            CardDef::new("Opponent", "{0}", "Instant", "Target opponent loses 1 life.", None).unwrap(),
            CardDef::new("Anyone", "{0}", "Instant", "Target player loses 1 life.", None).unwrap(),
        ];
        let mut g = Game::new(defs, 2, 20);
        if controller == 1 { g.act(0, Move::Pass).unwrap(); }
        let opponent = g.put(controller, 0, Zone::Hand);
        let anyone = g.put(controller, 1, Zone::Hand);
        let before = format!("{g:?}");
        cite!(["102.2", "115.1"]; refused("an illegal target, you: you aren't an opponent (115.1)"): g.act(controller, Move::Cast { card: opponent, targets: vec![vec![Target::Player(controller)]] }));
        assert_eq!(format!("{g:?}"), before);
        g.act(controller, Move::Cast { card: opponent, targets: vec![vec![Target::Player(1-controller)]] }).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        cite!(["102.2"]; case: format!("controller {controller}"); assert_eq!(g.players[1-controller].life, 19));
        while g.priority != controller { g.act(g.priority, Move::Pass).unwrap(); }
        g.act(controller, Move::Cast { card: anyone, targets: vec![vec![Target::Player(controller)]] }).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        assert_eq!(g.players[controller].life, 19);
    }
}
