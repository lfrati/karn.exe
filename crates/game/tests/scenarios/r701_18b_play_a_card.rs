//! "When you play a card" (701.18b): to play a card is to play it as a land or to cast it as a
//! spell, so casting a spell is playing a card, and a phrase about playing a land still asks
//! for a land.
use crate::support::{Board, cite, mana_pool};
use game::{Move, Zone};

#[test]
fn r701_18b_casting_a_spell_is_playing_a_card() {
    // Juju Bubble: "When you play a card, sacrifice this artifact."
    let mut t = Board::new(2).battlefield(0, &["Juju Bubble"]).hand(0, &["Grizzly Bears"]).build();
    t.mana(0, "{1}{G}");
    cite!(["701.18b", "601.2i"]; ok: t.cast(0, "Grizzly Bears", vec![], mana_pool("{1}{G}")));
    cite!(["603.3"]; assert_eq!(t.g.stack.len(), 2, "the trigger waits above the Bears"));
    t.resolve();
    t.expect().rules(["701.18b", "603.2"])
        .zone_exact(0, Zone::Graveyard, &["Juju Bubble"])
        .zone_exact(0, Zone::Battlefield, &["Grizzly Bears"]).stack(&[]).check();
}

#[test]
fn r701_18b_playing_a_land_is_playing_a_card() {
    let mut t = Board::new(2).battlefield(0, &["Juju Bubble"]).hand(0, &["Forest"]).build();
    let land = t.find_in(0, Zone::Hand, "Forest");
    cite!(["701.18b", "305.1"]; ok: t.g.act(0, Move::PlayLand(land)));
    t.resolve();
    t.expect().rules(["701.18b", "603.2"])
        .zone_exact(0, Zone::Graveyard, &["Juju Bubble"])
        .zone_exact(0, Zone::Battlefield, &["Forest"]).check();
}

#[test]
fn r701_18b_another_players_spell_is_not_your_play() {
    let mut t = Board::new(2).battlefield(0, &["Juju Bubble"]).hand(1, &["Lightning Bolt"]).build();
    t.g.act(0, Move::Pass).unwrap();
    t.mana(1, "{R}");
    cite!(["701.18b"]; ok: t.cast(1, "Lightning Bolt", vec![crate::support::player(0)], mana_pool("R")));
    cite!(["603.2"]; assert_eq!(t.g.stack.len(), 1, "only the Bolt: its caster is not Juju Bubble's controller"));
    t.resolve();
    t.expect().rules(["603.2"]).zone_exact(0, Zone::Battlefield, &["Juju Bubble"]).life(0, 17).check();
}

#[test]
fn r701_18b_a_cast_spell_is_not_a_land_played() {
    // Horn of Greed: "Whenever a player plays a land, that player draws a card." A land-only
    // play phrase keeps asking for a land when a spell is cast.
    let mut t = Board::new(2).battlefield(0, &["Horn of Greed"]).hand(0, &["Grizzly Bears", "Forest"]).fill(5).build();
    t.mana(0, "{1}{G}");
    cite!(["701.18b"]; ok: t.cast(0, "Grizzly Bears", vec![], mana_pool("{1}{G}")));
    cite!(["603.2"]; assert_eq!(t.g.stack.len(), 1, "casting the Bears plays no land"));
    t.resolve();
    let land = t.find_in(0, Zone::Hand, "Forest");
    cite!(["305.1"]; ok: t.g.act(0, Move::PlayLand(land)));
    cite!(["701.18b", "603.2"]; assert_eq!(t.g.stack.len(), 1, "playing the Forest triggers the Horn"));
}
