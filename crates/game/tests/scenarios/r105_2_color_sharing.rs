//! Color sharing reads the chosen creature's effective colors and excludes that target.
use crate::support::{cite, mana_pool, oracle, paid};
use game::{Game, Move, Target, Zone};
use game::state::{Answer, Choice, Decision, Placement};

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
    cite!(["120.3e"]; assert_eq!(g.obj(target).damage, 2, "the target is excluded from the additional group"));
    cite!(["404.3"]; assert_eq!(g.decision, Some(Decision::Choose { player: 1,
        choice: Choice::OrderCards { to: Placement::Zone(Zone::Graveyard), cards: vec![target, hexproof] } })));
    g.act(1, Move::Choose(Answer::Objects(vec![hexproof, target]))).unwrap();
    cite!(["105.2", "115.1a", "704.5g"]; assert!([target, own, hexproof].iter().all(|o| !g.battlefield.contains(o))));
    cite!(["105.2"]; assert!(g.battlefield.contains(&red)));
    cite!(["105.2"]; assert_eq!(g.obj(red).damage, 0));
}

#[test]
fn r105_2_spreading_plague_compares_to_the_entering_creature_and_excludes_it() {
    let mut g = Game::new(vec![oracle("Spreading Plague"), oracle("Grizzly Bears"), oracle("Gladecover Scout"), oracle("Hill Giant")], 2, 20);
    g.put(0, 0, Zone::Battlefield);
    let old = g.put(0, 1, Zone::Battlefield);
    let hexproof = g.put(1, 2, Zone::Battlefield);
    let red = g.put(1, 3, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Hand);
    g.players[0].pool = mana_pool("GG");
    g.act(0, paid("GG", Move::Cast { card: bear, targets: vec![] })).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    let entering = *g.battlefield.last().unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["105.2", "115.1d", "701.7a"]; assert!([old, hexproof].iter().all(|o| !g.battlefield.contains(o))));
    cite!(["105.2", "608.2c"]; assert!(g.battlefield.contains(&red) && g.battlefield.contains(&entering)));
}

#[test]
fn r105_2_wojek_siren_pumps_the_targets_color_group_without_targeting_it_again() {
    let mut g = Game::new(vec![oracle("Wojek Siren"), oracle("Grizzly Bears"), oracle("Gladecover Scout"), oracle("Hill Giant")], 2, 20);
    let target = g.put(1, 1, Zone::Battlefield);
    let hexproof = g.put(1, 2, Zone::Battlefield);
    let red = g.put(1, 3, Zone::Battlefield);
    let spell = g.put(0, 0, Zone::Hand);
    g.players[0].pool = mana_pool("W");
    g.act(0, paid("W", Move::Cast { card: spell, targets: vec![vec![Target::Object(target)]] })).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["105.2", "613.4c"]; assert_eq!((g.power(target), g.power(hexproof), g.power(red)), (Some(3), Some(2), Some(3))));
}
