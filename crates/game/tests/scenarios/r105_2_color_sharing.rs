//! Color sharing reads the chosen creature's effective colors and excludes that target.
use crate::support::{cite, mana_pool, oracle, paid};
use game::{Game, Move, Target, Zone};

#[test]
fn r105_2_cleansing_beam_uses_color_sharing_without_targeting_other_recipients() {
    let mut g = Game::new(vec![oracle("Cleansing Beam"), oracle("Grizzly Bears"), oracle("Gladecover Scout"), oracle("Hill Giant")], 2, 20);
    let target = g.put(1, 1, Zone::Battlefield);
    let own = g.put(0, 1, Zone::Battlefield);
    let hexproof = g.put(1, 2, Zone::Battlefield);
    let red = g.put(1, 3, Zone::Battlefield);
    let beam = g.put(0, 0, Zone::Hand);
    g.players[0].pool = mana_pool("RRRRR");
    g.act(0, paid("RRRRR", Move::Cast { card: beam, targets: vec![vec![Target::Object(target)]] })).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["105.2", "115.1a", "704.5g"]; assert!([target, own, hexproof].iter().all(|o| !g.battlefield.contains(o))));
    cite!(["105.2"]; assert!(g.battlefield.contains(&red)));
    cite!(["105.2"]; assert_eq!(g.obj(red).damage, 0));
}
