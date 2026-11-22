//! Optional counter effects may happen in part; a referenced payment requires the whole instruction.
use crate::support::{cite, oracle};
use game::{CardDef, Game, Move, Zone};
use game::state::{Answer, Choice, Decision};

#[test]
fn r118_12_optional_counter_payment_checks_every_holder_but_plain_effect_can_place_partially() {
    for payment in [false, true] {
        let mut text = "You may put a +1/+1 counter on each creature you control.".to_owned();
        if payment { text.push_str(" If you do, draw a card."); }
        let effect = CardDef::new("Optional placement", "{0}", "Sorcery", &text, None).unwrap();
        let ban = CardDef::new("Artifact counter ban", "{0}", "Enchantment", "Artifacts can't have counters put on them.", None).unwrap();
        for def in [&effect, &ban] { game::play::check(def).unwrap(); }
        let mut g = Game::new(vec![effect, ban, oracle("Grizzly Bears"), oracle("Ornithopter"), oracle("Forest")], 2, 20);
        g.put(0, 1, Zone::Battlefield);
        let bear = g.put(0, 2, Zone::Battlefield);
        let artifact = g.put(0, 3, Zone::Battlefield);
        g.put(0, 4, Zone::Library);
        let card = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        if payment {
            cite!(["118.3", "118.12"]; assert!(g.decision.is_none() && g.players[0].hand.is_empty()));
            assert!(g.counters(bear).is_empty());
        } else {
            cite!(["609.3"]; assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } })));
            g.act(0, Move::Choose(Answer::Yes)).unwrap();
            cite!(["609.3"]; assert_eq!(g.counters(bear).get("+1/+1"), Some(&1)));
        }
        cite!(["101.2"]; assert!(g.counters(artifact).is_empty()));
    }
}
