//! A linked card's owner controls its optional cast even when another player owns the trigger.
use crate::support::{cite, oracle, mana_pool, paid};
use game::{Game, Move, Target, Zone};
use game::state::{Answer, Choice, Decision};
fn pass(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }

#[test]
fn r607_2a_spell_queller_offers_its_linked_card_to_that_cards_owner() {
    let mut g = Game::new(vec![oracle("Spell Queller"), oracle("Lightning Bolt"), oracle("Unsummon")], 2, 20);
    g.active = 1; g.priority = 1;
    let bolt = g.put(1, 1, Zone::Hand);
    g.players[1].pool = mana_pool("R");
    g.act(1, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Player(0)]] })).unwrap();
    let bolt = *g.stack.last().unwrap();
    g.act(1, Move::Pass).unwrap();
    let queller = g.put(0, 0, Zone::Hand);
    g.players[0].pool = mana_pool("CWU");
    g.act(0, paid("CWU", Move::Cast { card: queller, targets: vec![] })).unwrap();
    pass(&mut g);
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bolt)]])).unwrap();
    pass(&mut g);
    cite!(["607.2a", "701.13a"]; assert_eq!(g.exile.len(), 1));
    let exiled = g.exile[0];
    let queller = g.battlefield[0];
    let bounce = g.put(1, 2, Zone::Hand);
    g.players[1].pool = mana_pool("U");
    // The active player receives priority again after the entry trigger resolves.
    g.act(1, paid("U", Move::Cast { card: bounce, targets: vec![vec![Target::Object(queller)]] })).unwrap();
    pass(&mut g); pass(&mut g);
    cite!(["607.2a", "608.2g"]; assert_eq!(g.decision,
        Some(Decision::Choose { player: 1, choice: Choice::Cast { cards: vec![exiled] } })));
    let mut decline = g.clone();
    decline.act(1, Move::Choose(Answer::No)).unwrap();
    cite!(["608.2g"]; assert_eq!(decline.exile, vec![exiled]));
    g.act(1, Move::Cast { card: exiled, targets: vec![vec![Target::Player(0)]] }).unwrap();
    pass(&mut g);
    cite!(["608.2g", "118.9"]; assert_eq!(g.players[0].life, 17));
    cite!(["400.3"]; assert!(g.exile.is_empty()));
}
