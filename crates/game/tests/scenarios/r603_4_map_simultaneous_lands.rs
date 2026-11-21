//! Archaeomancer's Map ("Whenever a land an opponent controls enters, if that player controls
//! more lands than you, you may put a land card from your hand onto the battlefield") when an
//! opponent's two lands enter at once (Explosive Vegetation: "Search your library for up to
//! two basic land cards, put them onto the battlefield tapped, then shuffle").
//! 603.2c: one event with two lands entering triggers it twice. 603.4: each checks the
//! condition again as it resolves, so the first one's land can stop the second.
use crate::support;
use support::*;
use game::state::Answer;

/// The Map's controller (player 0) has two Plains; player 1 casts Explosive Vegetation and
/// fetches a Forest and a Mountain.
fn vegetation(islands: usize) -> T {
    let islands = vec!["Island"; islands];
    let mut t = Board::new(2).battlefield(0, &["Archaeomancer's Map", "Plains", "Plains"]).hand(0, &["Plains", "Plains"])
        .battlefield(1, &islands).hand(1, &["Explosive Vegetation"]).library(1, &["Forest", "Mountain"]).build();
    t.g.active = 1;
    t.g.priority = 1;
    t.mana(1, "{3}{G}").cast(1, "Explosive Vegetation", vec![], crate::support::mana_pool("CCCG")).unwrap();
    t.g.act(1, Move::Pass).unwrap();
    t.g.act(0, Move::Pass).unwrap();
    let Some((1, Asked::Search { among, .. })) = t.g.asked() else { panic!("{:?}", t.g.asked()) };
    let mut offered: Vec<_> = among.iter().map(|o| t.g.obj(*o).chars.name.as_str()).collect();
    offered.sort();
    assert_eq!(offered, vec!["Forest", "Mountain"]);
    t.answer(1, Answer::Objects(among)).unwrap();
    t
}

fn lands(t: &T, p: usize) -> usize {
    t.g.battlefield.iter().filter(|o| t.g.controller(**o) == p && t.g.seen(**o, None).types.contains(&ir::CardType::Land)).count()
}

/// Then pass priority and accept each offered Map instruction with one selected land.
fn resolve_saying_yes(t: &mut T) -> usize {
    let mut asked = 0;
    for _ in 0..20 {
        match t.g.asked() {
            Some((0, Asked::OptionalObjects { among, .. })) => { asked += 1; t.answer(0, Answer::OptionalObjects(Some(vec![among[0]]))).unwrap(); }
            Some((_, Asked::Priority)) if t.g.stack.is_empty() => return asked,
            Some((p, Asked::Priority)) => { t.g.act(p, Move::Pass).unwrap(); }
            other => panic!("{other:?}"),
        }
    }
    panic!("did not settle");
}

#[test]
fn r603_2c_two_lands_entering_together_trigger_the_map_twice_and_both_resolve() {
    let mut t = vegetation(2);
    let Some((0,Asked::Order(abilities)))=t.g.asked() else {panic!("expected two Map trigger subjects: {:?}",t.g.asked())};
    cite!(["603.3b"]; assert_eq!(abilities.len(),2));
    // Both entrants belong to the same opponent; either chosen order preserves
    // the independently rechecked intervening condition of each occurrence.
    t.g.act(0,Move::Order(vec![0,1])).unwrap();
    assert_eq!(lands(&t, 1), 4);
    cite!(["603.2c"]; assert_eq!(t.g.stack.len(), 2, "one trigger per land"));
    assert_eq!(resolve_saying_yes(&mut t), 2);
    cite!(["603.4"]; assert_eq!((lands(&t, 0), t.g.players[0].hand.len()), (4, 0), "4 > 3 still holds for the second"));
}

#[test]
fn r603_4_the_first_maps_land_ends_the_seconds_condition() {
    let mut t = vegetation(1);
    let Some((0,Asked::Order(abilities)))=t.g.asked() else {panic!("expected two Map trigger subjects: {:?}",t.g.asked())};
    cite!(["603.3b"]; assert_eq!(abilities.len(),2));
    // Both entrants belong to the same opponent; either chosen order preserves
    // the independently rechecked intervening condition of each occurrence.
    t.g.act(0,Move::Order(vec![0,1])).unwrap();
    assert_eq!(lands(&t, 1), 3);
    cite!(["603.2c", "603.4"]; assert_eq!(t.g.stack.len(), 2, "3 > 2 when both lands entered"));
    cite!(["603.4"]; assert_eq!(resolve_saying_yes(&mut t), 1, "after the first, 3 is not more than 3: the second does nothing"));
    assert_eq!((lands(&t, 0), t.g.players[0].hand.len()), (3, 1));
}
