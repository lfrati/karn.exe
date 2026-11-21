//! Exile face down (406.3): the engine exiles face down the top cards of a library, which
//! nobody looks at, and one of the cards a player just looked at, which that player may go on
//! looking at. Other face-down exiles, and playing a card exiled face down (406.3a), are refused
//! with the reason named.
use crate::support::{oracle, oracle_read};
use game::CardDef;

fn refused(name: &str) -> String {
    game::play::check(&oracle_read(name)).map_err(|u| u.0).expect_err(name)
}

#[test]
fn r406_3_other_face_down_exiles_are_refused() {
    // from a hand, after a search, of cards named by "them", every card of a library
    for name in ["Gustha's Scepter", "Hoarding Broodlord", "Scarlet Witch, Chaotic Avenger", "Inverter of Truth", "Scroll Rack"] {
        assert!(refused(name).contains("exile face down"), "{name}: {}", refused(name));
    }
    for text in ["Reveal the top two cards of your library. Exile one of them face down and put the rest into your hand."] {
        let d = CardDef::new("Procedure", "{0}", "Sorcery", text, None).unwrap();
        let reason = game::play::check(&d).map_err(|u| u.0).expect_err(text);
        assert!(reason.contains("not looked at"), "{text}: {reason}");
    }
}

#[test]
fn r406_3a_hideaway_cards_that_play_the_exiled_card_are_refused() {
    // the play-without-paying half is a later capability, with turning the card face up
    for name in ["Mosswort Bridge", "Fight Rigging", "Clive's Hideaway"] {
        assert_eq!(refused(name), "program: action not lowered: Cast", "{name}");
    }
    game::play::check(&oracle("Watcher for Tomorrow")).expect("hideaway with a return to hand");
}
