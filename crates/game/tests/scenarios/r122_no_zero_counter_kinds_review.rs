//! Proliferate offers holders that have a
//! counter (701.34a). Through rules paths, a holder never keeps a kind at zero: an entry
//! made for zero counters adds none, and removing the last counter of a kind removes the
//! kind. So "has a counter" equals "has a kind in its map" here; only a fixture could
//! build a zero entry.
use game::{CardDef, Game, Move, Zone};

#[test]
fn an_unkicked_everflowing_chalice_and_a_removed_last_counter_leave_no_zero_kind() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let chalice = CardDef::from_faces(CardDef::catalog(&faces).get("Everflowing Chalice").unwrap()).unwrap();
    game::play::check(&chalice).unwrap_or_else(|e| panic!("{}", e.0));
    let mut g = Game::new(vec![chalice], 2, 20);
    g.players[0].pool.colors = [5; 5];
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    for _ in 0..4 { if g.stack.is_empty() { break } g.act(g.priority, Move::Pass).unwrap(); }
    let [entered] = g.battlefield[..] else { panic!("{:?}", g.battlefield) };
    assert!(g.counters(entered).is_empty(), "kicked zero times: {:?}", g.counters(entered));

    let other = g.put(0, 0, Zone::Battlefield);
    g.put_counters(other, "charge", 1);
    assert_eq!(g.remove_counters(other, "charge", 5), 1);
    assert!(g.counters(other).is_empty());
    g.put_counters(game::Target::Player(0), "poison", 1);
    g.remove_counters(game::Target::Player(0), "poison", 1);
    assert!(g.counters(game::Target::Player(0)).is_empty());
}
