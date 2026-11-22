//! A finite card-type choice consumes each retained card once, before one simultaneous move.
use crate::support::{cite, mana_pool, oracle, paid};
use game::{Game, Move, Zone};
use game::state::{Answer, Choice, Decision};

#[test]
fn r608_2d_atraxa_assigns_multitype_cards_and_moves_all_choices_together() {
    for artifact_creature in [true, false] {
        let names = ["Atraxa, Grand Unifier", "Ornithopter", "Grizzly Bears", "Sol Ring", "Tarfire", "Lightning Bolt", "Rampant Growth", "Forest", "Phyrexian Arena", "Liliana of the Veil", "Island"];
        let mut g = Game::new(names.into_iter().map(oracle).collect::<Vec<_>>(), 2, 20);
        let hidden = g.put(0, 10, Zone::Library);
        let cards: Vec<_> = (1..=10).map(|def| g.put(0, def, Zone::Library)).collect();
        let source = g.put(0, 0, Zone::Hand);
        g.players[0].pool = mana_pool("CCCGWUB");
        g.act(0, paid("CCCGWUB", Move::Cast { card: source, targets: vec![] })).unwrap();
        for _ in 0..4 { g.act(g.priority, Move::Pass).unwrap(); }
        // Card text/ruling: the artifact creature may be assigned to either type.
        // Kindred is a type even though the older reminder text omits it.
        let selections = if artifact_creature { [0, 1, 7, 4, 3, 6, 8, 5] } else { [2, 0, 7, 4, 3, 6, 8, 5] };
        for (index, selected) in selections.iter().enumerate() {
            let Some(Decision::Choose { player: 0, choice: Choice::Select { among, min: 0, max: 1 } }) = &g.decision else { panic!("choice {index}: {:?}", g.decision) };
            cite!(["608.2d"]; assert!(among.contains(&cards[*selected])));
            cite!(["608.2c"]; assert!(cards.iter().all(|card| g.obj(*card).zone == Zone::Library)));
            if index == 1 && artifact_creature {
                cite!(["608.2d"]; assert!(!among.contains(&cards[0])));
                cite!(["608.2d"]; refused("not among the offered objects"): g.act(0, Move::Choose(Answer::Objects(vec![cards[0]]))));
            }
            g.act(0, Move::Choose(Answer::Objects(vec![cards[*selected]]))).unwrap();
        }
        cite!(["608.2c"]; assert_eq!(g.players[0].hand.len(), 8));
        // Public cards randomly reordered into the library receive fresh opaque
        // handles. Check exact remaining names, including the unrevealed eleventh.
        let mut remaining: Vec<_> = g.players[0].library.iter().map(|o| g.obj(*o).chars.name.as_str()).collect();
        remaining.sort_unstable();
        let mut expected = vec!["Island", "Island", if artifact_creature { "Sol Ring" } else { "Grizzly Bears" }];
        expected.sort_unstable();
        cite!(["608.2c"]; assert_eq!(remaining, expected));
        cite!(["608.2c"]; assert!(g.players[0].library.contains(&hidden)));
        cite!(["608.2c"]; assert!(g.decision.is_none() && g.stack.is_empty()));
    }
}

#[test]
fn r609_3_atraxa_handles_a_short_or_empty_library_and_declined_types() {
    for count in [0, 2] {
        let mut g = Game::new(vec![oracle("Atraxa, Grand Unifier"), oracle("Forest")], 2, 20);
        for _ in 0..count { g.put(0, 1, Zone::Library); }
        let source = g.put(0, 0, Zone::Hand);
        g.players[0].pool = mana_pool("CCCGWUB");
        g.act(0, paid("CCCGWUB", Move::Cast { card: source, targets: vec![] })).unwrap();
        for _ in 0..4 { g.act(g.priority, Move::Pass).unwrap(); }
        if count > 0 {
            let Some(Decision::Choose { player: 0, choice: Choice::Select { among, min: 0, max: 1 } }) = &g.decision else { panic!("{:?}", g.decision) };
            cite!(["609.3"]; assert_eq!(among.len(), count));
            cite!(["608.2d"]; ok: g.act(0, Move::Choose(Answer::Objects(vec![]))));
        }
        cite!(["609.3"]; assert_eq!(g.players[0].library.len(), count));
        cite!(["608.2d"]; assert!(g.players[0].hand.is_empty()));
        cite!(["608.2c"]; assert!(g.decision.is_none() && g.stack.is_empty()));
    }
}
