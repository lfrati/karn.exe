//! Cloak and manifest-dread boundary/retention checks.
use crate::support::*;
use game::face::Way;

#[test]
fn r701_62a_an_empty_or_singleton_library_does_not_invent_a_selection() {
    for library in [&[][..], &["Island"][..]] {
        let procedure = CardDef::new("Procedure", "{0}", "Sorcery", "Manifest dread twice.", None).unwrap();
        game::play::check(&procedure).unwrap();
        let mut t = Board::new(2).library(0, library).library(1, &["Forest"]).build();
        let def = t.g.defs.len();
        std::sync::Arc::make_mut(&mut t.g.defs).push(procedure);
        let card = t.g.put(0, def, Zone::Hand);
        t.g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
        t.resolve();
        cite!(["701.62a"]; case: library.len(); assert!(t.g.decision.is_none()));
        cite!(["701.62a"]; case: library.len(); assert_eq!(t.g.battlefield.len(), library.len()));
        assert!(t.g.players[0].library.is_empty());
        assert!(!t.g.players[0].lost);
        cite!(["701.62a"]; case: library.len(); assert_eq!(t.g.players[0].graveyard.len(), 1, "only the Procedure itself"));
    }
}

#[test]
fn r701_58c_cloaked_megamorph_turn_up_has_the_alternative_counter_only() {
    for morph in [false, true] {
        let procedure =
            CardDef::new("Procedure", "{0}", "Sorcery", "Cloak the top card of your library.", None).unwrap();
        game::play::check(&procedure).unwrap();
        let mut t = Board::new(2).library(0, &["Ainok Survivalist"]).fill(8).build();
        let def = t.g.defs.len();
        std::sync::Arc::make_mut(&mut t.g.defs).push(procedure);
        let card = t.g.put(0, def, Zone::Hand);
        t.g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
        t.resolve();
        let permanent = t.g.battlefield[0];
        t.mana(0, "{1}{G}");
        let mut copy = t.g.clone();
        for g in [&mut t.g, &mut copy] {
            g.act(0, crate::support::paid("CG", Move::TurnFaceUp(permanent, if morph { Way::Morph } else { Way::Cloaked }))).unwrap();
            cite!(["701.58c"]; case: morph; assert_eq!(g.obj(permanent).chars.name, "Ainok Survivalist"));
            cite!(["701.58c", "702.37b"]; case: morph; assert_eq!(g.counters(permanent).get("+1/+1").copied().unwrap_or(0), u32::from(morph)));
            assert_eq!(g.players[0].pool.total(), 0);
        }
    }
}

#[test]
fn r701_58d_a_cloaked_disguise_card_turns_up_by_either_route() {
    for way in [Way::Cloaked, Way::Disguise] {
        let procedure =
            CardDef::new("Procedure", "{0}", "Sorcery", "Cloak the top card of your library.", None).unwrap();
        game::play::check(&procedure).unwrap();
        let mut t = Board::new(2).library(0, &["Exit Specialist"]).fill(8).build();
        let def = t.g.defs.len();
        std::sync::Arc::make_mut(&mut t.g.defs).push(procedure);
        let card = t.g.put(0, def, Zone::Hand);
        t.g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
        t.resolve();
        let permanent = t.g.battlefield[0];
        t.mana(0, "{1}{U}");
        t.g.act(0, crate::support::paid("CU", Move::TurnFaceUp(permanent, way))).unwrap();
        cite!(["701.58d"]; case: format!("{way:?}"); assert_eq!(t.g.obj(permanent).chars.name, "Exit Specialist"));
        assert_eq!(t.g.players[0].pool.total(), 0);
        cite!(["701.58d"]; case: format!("{way:?}"); assert!(!t.g.face_down(permanent)));
    }
}
