//! CR 101.2 and 601.3: a prohibition on casting from some zones overrides every permission to
//! cast from them, and leaves casting from the other zones alone.
use crate::support::{Board, cite, paid};
use game::announcement::CastRoute;
use game::{Move, Zone};

/// Soulless Jailer: "Permanent cards in graveyards can't enter the battlefield. Players can't
/// cast noncreature spells from graveyards or exile." A noncreature spell is refused from a
/// graveyard (flashback) and from exile (Act on Impulse), a creature spell from exile is not,
/// and nothing stops a noncreature spell cast from a hand.
#[test]
fn r601_3_soulless_jailer_refuses_noncreature_spells_from_graveyards_and_exile() {
    let mut t = Board::new(2)
        .battlefield(1, &["Soulless Jailer"])
        .graveyard(0, &["Think Twice"])
        .hand(0, &["Act on Impulse"])
        .library(0, &["Lightning Bolt", "Grizzly Bears", "Wastes", "Wastes"])
        .build();
    let twice = t.find_in(0, Zone::Graveyard, "Think Twice");
    t.mana(0, "{2}{U}");
    cite!(["601.3", "702.34a"]; refused("an effect prohibits casting that spell (601.3)"):
        t.g.act(0, paid("CCU", Move::CastVia(CastRoute::Flashback, Box::new(Move::Cast { card: twice, targets: vec![] })))));
    t.g.players[0].pool = Default::default();
    t.mana(0, "{2}{R}").cast(0, "Act on Impulse", vec![], crate::support::mana_pool("CCR")).unwrap().resolve();
    let bolt = t.find_in(0, Zone::Exile, "Lightning Bolt");
    let bears = t.find_in(0, Zone::Exile, "Grizzly Bears");
    t.mana(0, "{R}");
    cite!(["601.3"]; refused("an effect prohibits casting that spell (601.3)"):
        t.g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![game::Target::Player(1)]] })));
    t.g.players[0].pool = Default::default();
    t.mana(0, "{1}{G}");
    cite!(["601.3"]; ok: t.g.act(0, paid("CG", Move::Cast { card: bears, targets: vec![] })));
    t.resolve();
    // the creature spell enters from the stack, not from a graveyard (614.17)
    cite!(["601.3", "614.17"]; assert!(t.g.battlefield.iter().any(|o| t.g.obj(*o).chars.name == "Grizzly Bears")));
}

/// Drannith Magistrate: "Your opponents can't cast spells from anywhere other than their
/// hands." An opponent can't cast a spell from their graveyard but may from their hand; its
/// controller may still cast from a graveyard.
#[test]
fn r601_3_drannith_magistrate_leaves_opponents_only_their_own_hands() {
    let mut t = Board::new(2)
        .active(1)
        .battlefield(0, &["Drannith Magistrate"])
        .graveyard(1, &["Think Twice"])
        .graveyard(0, &["Think Twice"])
        .hand(1, &["Lightning Bolt"])
        .library(0, &["Wastes", "Wastes"])
        .library(1, &["Wastes", "Wastes"])
        .build();
    let theirs = t.find_in(1, Zone::Graveyard, "Think Twice");
    t.mana(1, "{2}{U}");
    cite!(["601.3", "702.34a"]; refused("an effect prohibits casting that spell (601.3)"):
        t.g.act(1, paid("CCU", Move::CastVia(CastRoute::Flashback, Box::new(Move::Cast { card: theirs, targets: vec![] })))));
    t.g.players[1].pool = Default::default();
    t.mana(1, "{R}");
    cite!(["601.3"]; ok: t.g.act(1, paid("R", Move::Cast { card: t.find_in(1, Zone::Hand, "Lightning Bolt"), targets: vec![vec![game::Target::Player(0)]] })));
    t.resolve();
    cite!(["601.3"]; assert_eq!(t.g.players[0].life, 17));
    // its controller is no opponent: it may cast from its graveyard
    t.g.act(1, Move::Pass).unwrap();
    let mine = t.find_in(0, Zone::Graveyard, "Think Twice");
    t.mana(0, "{2}{U}");
    cite!(["601.3", "702.34a"]; ok: t.g.act(0, paid("CCU", Move::CastVia(CastRoute::Flashback, Box::new(Move::Cast { card: mine, targets: vec![] })))));
}
