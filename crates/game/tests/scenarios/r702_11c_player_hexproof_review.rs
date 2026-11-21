//! Player hexproof (702.11c)
//! is enforced wherever it is admitted. Run with `r702::r702_11c_player_hexproof_review::`.
use crate::support::cite;
use std::sync::OnceLock;
use game::{CardDef, Game, Move, Target, Zone};

fn oracle(name: &str) -> CardDef {
    static FACES: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    let faces = FACES.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    CardDef::from_faces(CardDef::catalog(faces).get(name).unwrap()).unwrap()
}

/// Pinned Leyline of Sanctity ("You have hexproof.") passes `play::check`, so an
/// opponent must not be able to target its controller (702.11c, 115.1, 601.2c): player
/// hexproof is enforced, or else the card refused. The synthetic "You have shroud." and
/// "Players have hexproof." have the same shape.
#[test]
fn admitted_player_hexproof_stops_an_opponent_from_targeting_its_controller() {
    let (leyline, bolt) = (oracle("Leyline of Sanctity"), oracle("Lightning Bolt"));
    assert_eq!(game::play::check(&leyline).map_err(|e| e.0), Ok(()));
    let mut g = Game::new(vec![leyline, bolt], 2, 20);
    g.players[0].pool.colors = [10; 5];
    g.put(1, 0, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Hand);
    cite!(["702.11c"]; refused("an illegal target, that player: that player has hexproof (702.11c)"): g.act(0, Move::Cast { card, targets: vec![vec![Target::Player(1)]] }));
}
