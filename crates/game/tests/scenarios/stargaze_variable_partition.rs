//! Native variable result partition; expectations from Oracle and pinned Stargaze rulings
//! 4c361e1a604f8e84 and ff89c3396891056d, never from a replay.
use crate::support::cite;
use game::{CardDef, Game, Move, Zone};
use game::state::{Answer, Choice, Decision};

fn setup(player: usize, size: usize, x: u32) -> (Game, Vec<game::ObjId>, game::ObjId) {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs: Vec<CardDef> = ["Stargaze", "Island", "Forest", "Mountain", "Plains", "Swamp"].iter()
        .map(|n| CardDef::from_faces(&catalog[*n]).unwrap()).collect();
    let mut g = Game::new(defs, 2, 20);
    g.active = player; g.priority = player;
    let mut cards = Vec::new();
    for i in (0..size).rev() { cards.push(g.put(player, i % 5 + 1, Zone::Library)); }
    cards.reverse();
    let spell = g.put(player, 0, Zone::Hand);
    g.players[player].pool.colors[2] = x + 2;
    (g, cards, spell)
}
fn cast(g: &mut Game, player: usize, spell: game::ObjId, x: u32) {
    g.act(player, Move::Paying(game::state::Pool { colors: [0, 0, x+2, 0, 0], ..Default::default() }, Box::new(Move::WithX(x, Box::new(Move::Cast { card: spell, targets: vec![] }))))).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
}
fn names(g: &Game, ids: &[game::ObjId]) -> Vec<String> {
    ids.iter().map(|id| g.obj(*id).chars.name.clone()).collect()
}

// Rules: 107.3a, 601.2b, 608.2c, 608.2d, 701.20e, 400.7, 119.3
// Both seats and both X=1 branches: no movement/life loss during the retained choice;
// exact selected incarnation and complement move once, with new zone identities.
#[test]
fn r608_both_seats_and_one_card_branches_retain_private_partition() {

    for player in 0..2 { for selected in 0..2 {
        let (mut g, cards, spell) = setup(player, 3, 1);
        let outsider = g.put(player, 5, Zone::Hand);
        cast(&mut g, player, spell, 1);
        let Some(Decision::Choose { player: chooser, choice: Choice::Objects { among, count, .. } }) = &g.decision else { panic!("{:?}", g.decision) };
        cite!(["608.2d"]; assert_eq!((*chooser, *count, among.clone()), (player, 1, cards[..2].to_vec())));
        cite!(["701.20e"]; assert!(cards[..2].iter().all(|c| g.sees(player, *c) && !g.sees(1-player, *c))));
        cite!(["701.20e"]; assert!(!g.sees(player, cards[2])));
        cite!(["701.20e"]; assert!(g.observations(1-player).is_empty()));
        cite!(["701.20e"]; assert_eq!(g.observations(player).len(), 1));
        cite!(["701.20e"]; assert_eq!(g.observations(player)[0].cards.iter().map(|c| c.object).collect::<Vec<_>>(), cards[..2]));
        let showing = g.observations(player)[0].clone();
        let before = format!("{g:?}");
        // Uncited (protocol): only the correlated chooser can answer; refusal is atomic.
        crate::support::refused(g.act(1-player, Move::Choose(Answer::Objects(vec![cards[0]]))),"that decision is another player's");
        assert_eq!(format!("{g:?}"), before);
        for (answer, reason) in [
            (vec![outsider], "not among"), (vec![cards[2]], "not among"),
            (vec![cards[0], cards[0]], "once"), (vec![], "exactly 1"),
            (vec![cards[0], cards[1]], "exactly 1"),
        ] {
            cite!(["608.2d"]; refused(reason): g.act(player, Move::Choose(Answer::Objects(answer))));
            // Uncited (transaction): malformed answer preserves every retained field.
            assert_eq!(format!("{g:?}"), before);
        }
        // Uncited (protocol): resolution gives nobody priority during suspension.
        crate::support::refused(g.act(player, Move::Pass),"a decision is pending: a choice of the effect being applied");
        assert_eq!(format!("{g:?}"), before);
        cite!(["608.2c"]; assert_eq!(names(&g, &g.players[player].library), vec!["Mountain", "Forest", "Island"]));
        cite!(["119.3"]; assert_eq!(g.players[player].life, 20));
        g.act(player, Move::Choose(Answer::Objects(vec![cards[selected]]))).unwrap();
        let chosen = if selected == 0 { "Island" } else { "Forest" };
        let rest = if selected == 0 { "Forest" } else { "Island" };
        cite!(["608.2c"]; assert_eq!(names(&g, &g.players[player].hand), vec!["Swamp", chosen]));
        cite!(["608.2c"]; assert_eq!(names(&g, &g.players[player].graveyard), vec![rest, "Stargaze"]));
        cite!(["608.2c"]; assert_eq!(g.players[player].library, vec![cards[2]]));
        cite!(["400.7"]; assert!(cards[..2].iter().all(|id| !g.players[player].hand.contains(id) && !g.players[player].graveyard.contains(id))));
        cite!(["107.3a", "119.3"]; assert_eq!(g.players[player].life, 19));
        cite!(["608.2c"]; assert!(g.stack.is_empty() && g.decision.is_none()));
        // The frozen look still names the old library incarnations, delivered once.
        cite!(["701.20e", "400.7"]; assert_eq!(g.observations(player), &[showing]));
        cite!(["701.20e"]; assert!(!g.sees(1-player, *g.players[player].hand.last().unwrap())));
    }}
}

// Rules: 107.3a, 107.3i, 601.2b, 608.2c, 608.2d, 609.3, 119.3, 701.20e
// Rulings: short library changes the available partition, never announced X or life loss.
#[test]
fn r609_zero_normal_short_and_empty_libraries_keep_original_x() {
    for (x, size, selected, hand, grave, tail) in [
        (0, 3, vec![], vec![], vec!["Stargaze"], vec!["Island", "Forest", "Mountain"]),
        (2, 5, vec![0, 2], vec!["Island", "Mountain"], vec!["Forest", "Plains", "Stargaze"], vec!["Swamp"]),
        (2, 3, vec![0, 2], vec!["Island", "Mountain"], vec!["Forest", "Stargaze"], vec![]),
        (2, 2, vec![], vec!["Island", "Forest"], vec!["Stargaze"], vec![]),
        (2, 1, vec![], vec!["Island"], vec!["Stargaze"], vec![]),
        (2, 0, vec![], vec![], vec!["Stargaze"], vec![]),
    ] {
        let (mut g, cards, spell) = setup(0, size, x);
        cast(&mut g, 0, spell, x);
        if !selected.is_empty() {
            let Some(Decision::Choose { player: 0, choice: Choice::Objects { count: 2, among, .. } }) = &g.decision else { panic!("{:?}", g.decision) };
            cite!(["609.3", "608.2d"]; assert_eq!(among, &cards[..size.min(4)]));
            cite!(["119.3"]; assert_eq!(g.players[0].life, 20));
            g.act(0, Move::Choose(Answer::Objects(selected.iter().map(|i| cards[*i]).collect()))).unwrap();
        }
        if x == 2 && size == 5 {
            // CR404.3: explicitly authored owner order, separate from selection.
            assert_eq!(g.decision, Some(game::state::Decision::Choose { player: 0, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: vec![cards[1],cards[3]] } }));
            g.act(0, Move::Choose(game::state::Answer::Objects(vec![cards[3],cards[1]]))).unwrap();
        }
        cite!(["609.3", "608.2c"]; case: format!("X={x}, size={size}"); assert_eq!(names(&g, &g.players[0].hand), hand));
        cite!(["608.2c"]; assert_eq!(names(&g, &g.players[0].graveyard), grave));
        cite!(["608.2c"]; assert_eq!(names(&g, &g.players[0].library.iter().rev().copied().collect::<Vec<_>>()), tail));
        cite!(["107.3a", "107.3i", "119.3"]; assert_eq!(g.players[0].life, 20-x as i32));
        cite!(["609.3"]; assert!(g.stack.is_empty() && g.decision.is_none() && !g.finished()));
        cite!(["701.20e"]; assert!(g.observations(1).is_empty()));
        if x == 0 { cite!(["107.3a", "701.20e"]; assert!(g.observations(0).is_empty())); }
    }
}

#[test]
fn unrepresentable_twice_x_refuses_before_any_payment_or_disclosure() {
    // Uncited (engine boundary): 107.1b permits arbitrarily large positive integers;
    // this explicit refusal is a representation limit, never a Magic prohibition.
    for x in [2_147_483_648, u32::MAX-2] {
        let (mut g, _, spell) = setup(0, 3, x);
        let before = format!("{g:?}");
        let error = g.act(0, Move::Paying(game::state::Pool { colors: [0, 0, x+2, 0, 0], ..Default::default() }, Box::new(Move::WithX(x, Box::new(Move::Cast { card: spell, targets: vec![] }))))).unwrap_err();
        assert!(error.contains("numerical range"), "{error}");
        assert_eq!(format!("{g:?}"), before);
        assert!(g.observations(0).is_empty() && g.observations(1).is_empty());
    }
}

// Rules: 107.1b, 107.3a, 601.2b, 609.3, 119.3, 704.5a
// The largest X whose twice-X fits u32 remains legal; a one-card library goes
// wholly to hand and original-X life loss makes its controller lose the game.
#[test]
fn r107_largest_representable_twice_x_is_not_over_refused() {
    let x = 2_147_483_647;
    let (mut g, _, spell) = setup(0, 1, x);
    cast(&mut g, 0, spell, x);
    cite!(["609.3"]; assert_eq!(names(&g, &g.players[0].hand), vec!["Island"]));
    cite!(["608.2c"]; assert_eq!(names(&g, &g.players[0].graveyard), vec!["Stargaze"]));
    cite!(["107.3a", "119.3"]; assert_eq!(g.players[0].life, -2_147_483_627));
    cite!(["704.5a"]; assert!(g.finished() && g.players[0].lost));
}
