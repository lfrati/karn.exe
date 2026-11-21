//! 605.1a excludes loyalty abilities; their mana effect resolves through the stack.
//! Full pinned Chandra witness.
use crate::support::cite;
use game::{CardDef, Game, Move, Zone};

#[test]
fn full_chandra_loyalty_mana_uses_the_stack_and_allows_a_response() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let chandra = CardDef::from_faces(CardDef::catalog(&faces).get("Chandra, Novice Pyromancer").unwrap()).unwrap();
    game::play::check(&chandra).unwrap();
    let response = CardDef::new("Response", "{0}", "Instant", "You gain 1 life.", None).unwrap();
    let mut g = Game::new(vec![chandra, response], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.put_counters(source, "loyalty", 5);
    g.act(0, Move::Activate { source, ability: 1, targets: vec![], paying: vec![] }).unwrap();
    cite!(["606.4"]; assert_eq!(g.obj(source).counters.get("loyalty"), Some(&4), "cost paid on activation"));
    cite!(["605.1a"]; assert_eq!(g.stack.len(), 1));
    assert_eq!(g.players[0].pool.total(), 0, "no mana before resolution");
    g.act(0, Move::Pass).unwrap();
    assert_eq!(g.priority, 1);
    let card = g.put(1, 1, Zone::Hand);
    g.act(1, Move::Cast { card, targets: vec![] }).unwrap();
    cite!(["605.1a"]; assert_eq!(g.stack.len(), 2, "opponent may respond"));
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(g.players[1].life, 21);
    assert_eq!(g.stack.len(), 1);
    assert_eq!(g.players[0].pool.total(), 0);
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    assert!(g.stack.is_empty());
    assert_eq!(g.players[0].pool.colors[3], 2);
    assert_eq!(g.players[0].pool.total(), 2);
}
