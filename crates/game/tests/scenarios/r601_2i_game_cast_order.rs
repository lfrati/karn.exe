//! Whole-game cast order belongs to an accepted cast, not a turn or a proposed payment.
use crate::support::{cite, mana_pool, oracle, paid};
use game::{Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};
use game::announcement::CastCost;

fn game() -> Game {
    let mut g = Game::new(vec![oracle("Once Upon a Time"), oracle("Forest"),
        oracle("Grizzly Bears"), oracle("Counterspell"), oracle("Memnite")], 2, 20);
    // Bottom first. The untouched sixth card must remain above the returned five.
    for player in 0..2 { for _ in 0..8 { g.put(player, 1, Zone::Library); } }
    g
}
fn free(card: ObjId) -> Move {
    Move::CastFor(CastCost::Alternative(0), Box::new(Move::Cast { card, targets: vec![] }))
}
fn pass(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn decline(g: &mut Game) {
    pass(g);
    // CR 608.2d permits declining the whole reveal-and-put instruction.
    cite!(["608.2d"]; assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } }))));
    g.act(0, Move::Choose(Answer::No)).unwrap();
}
fn next_turn(g: &mut Game) {
    let turn = g.turn;
    for _ in 0..200 {
        if g.turn != turn && g.part == ir::Part::PrecombatMain && g.priority == g.active { return; }
        match g.decision {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            None => g.act(g.priority, Move::Pass).unwrap(),
            _ => panic!("unexpected choice: {:?}", g.decision),
        }
    }
    panic!("did not reach next turn");
}

#[test]
fn r601_2i_failed_proposal_land_and_opponent_cast_do_not_consume_your_first_spell() {
    let mut g = game();
    g.part = ir::Part::PrecombatMain;
    let land = g.put(0, 1, Zone::Hand);
    g.act(0, Move::PlayLand(land)).unwrap();
    let card = g.put(0, 0, Zone::Hand);
    cite!(["601.2h", "733.1"]; refused("your mana pool cannot pay that cost"): g.act(0, Move::Cast { card, targets: vec![] }));
    cite!(["601.2i"]; assert_eq!(g.players[0].game_casts, Some(0)));
    g.priority = 1;
    g.active = 1;
    let other = g.put(1, 4, Zone::Hand);
    g.act(1, Move::Cast { card: other, targets: vec![] }).unwrap();
    pass(&mut g);
    g.priority = 0;
    g.active = 0;
    cite!(["601.2i"]; assert_eq!((g.players[0].game_casts, g.players[1].game_casts), (Some(0), Some(1))));
    g.act(0, free(card)).unwrap();
    cite!(["118.9", "601.2i"]; assert_eq!(g.players[0].game_casts, Some(1)));
    decline(&mut g);
    cite!(["608.2c"]; assert!(g.stack.is_empty() && g.decision.is_none()));
}

#[test]
fn r601_2i_countered_spell_still_counts_on_later_turns_and_clones() {
    let mut g = game();
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, free(card)).unwrap();
    let spell = g.stack[0];
    g.act(0, Move::Pass).unwrap();
    let counter = g.put(1, 3, Zone::Hand);
    g.players[1].pool = mana_pool("UU");
    g.act(1, paid("UU", Move::Cast { card: counter, targets: vec![vec![Target::Object(spell)]] })).unwrap();
    pass(&mut g);
    cite!(["601.2i", "701.5a"]; assert_eq!(g.players[0].game_casts, Some(1)));
    next_turn(&mut g);
    next_turn(&mut g);
    let card = g.put(0, 0, Zone::Hand);
    let mut branch = g.clone();
    for state in [&mut g, &mut branch] {
        cite!(["118.9", "601.2i"]; refused("the condition for that alternative cost is not true"): state.act(0, free(card)));
        state.players[0].pool = mana_pool("CG");
        state.act(0, paid("CG", Move::Cast { card, targets: vec![] })).unwrap();
        decline(state);
        cite!(["601.2i"]; assert_eq!(state.players[0].game_casts, Some(2)));
    }
}

#[test]
fn r701_20a_real_once_selects_a_creature_or_land_and_returns_exactly_the_rest() {
    for chosen in [1usize, 2] {
        let mut g = Game::new(vec![oracle("Once Upon a Time"), oracle("Forest"), oracle("Grizzly Bears"), oracle("Counterspell")], 2, 20);
        g.put(0, 3, Zone::Library); // untouched sixth card
        for def in [3,1,3,2,3] { g.put(0, def, Zone::Library); }
        let untouched = g.players[0].library[0];
        let card = g.put(0, 0, Zone::Hand);
        g.act(0, free(card)).unwrap();
        pass(&mut g);
        g.act(0, Move::Choose(Answer::Yes)).unwrap();
        let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, count: 1, .. } }) = &g.decision else { panic!("{:?}", g.decision); };
        cite!(["608.2c", "701.20a"]; assert_eq!(among.len(), 2));
        let selected = *among.iter().find(|id| g.obj(**id).card == Some(chosen)).unwrap();
        g.act(0, Move::Choose(Answer::Objects(vec![selected]))).unwrap();
        cite!(["608.2c", "701.20a"]; assert_eq!(g.players[0].hand.iter().map(|id| g.obj(*id).card).collect::<Vec<_>>(), vec![Some(chosen)]));
        cite!(["401.2", "608.2c"]; assert_eq!(g.players[0].library.len(), 5));
        cite!(["401.2", "608.2c"]; assert_eq!(g.players[0].library.last(), Some(&untouched)));
        cite!(["608.2c"]; assert!(g.stack.is_empty() && g.decision.is_none()));
    }
}

#[test]
fn r608_2d_no_matching_card_still_offers_only_declining_a_private_option() {
    let mut g = Game::new(vec![oracle("Once Upon a Time"), oracle("Counterspell")], 2, 20);
    for _ in 0..3 { g.put(0, 1, Zone::Library); }
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, free(card)).unwrap();
    pass(&mut g);
    cite!(["608.2d"]; assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: false } }))));
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["608.2c"]; assert_eq!(g.players[0].library.len(), 3));
    cite!(["608.2c"]; assert!(g.players[0].hand.is_empty() && g.decision.is_none()));
}

#[test]
fn r601_2b_positioned_unknown_history_refuses_only_the_history_dependent_route() {
    let mut g = game();
    g.prepare_initial_position(3, ir::Part::PrecombatMain, 0, 0, 0, &[]).unwrap();
    g.set_initial_cast_history(&[]).unwrap();
    let card = g.put(0, 0, Zone::Hand);
    // Uncited (setup contract): absence of earlier-turn facts does not mean zero casts.
    assert_eq!(g.players[0].game_casts, None);
    cite!(["601.2b"]; refused("known whole-game cast count"): g.act(0, free(card)));
    g.players[0].pool = mana_pool("CG");
    g.act(0, paid("CG", Move::Cast { card, targets: vec![] })).unwrap();
    decline(&mut g);
    // Uncited (setup contract): ordinary accepted casts cannot invent a missing earlier total.
    assert_eq!(g.players[0].game_casts, None);
}

#[test]
fn r601_2i_supplied_totals_include_current_turn_casts_and_leave_failed_setup_unchanged() {
    let mut g = game();
    let card = g.put(0, 4, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    pass(&mut g);
    // Uncited (setup contract): an explicit total cannot contradict represented casts.
    assert_eq!(g.set_initial_game_cast_counts(&[Some(0),Some(0)]).unwrap_err(), "game cast total is smaller than the represented current-turn casts");
    cite!(["601.2i"]; assert_eq!(g.players[0].game_casts, Some(1)));
    let mut earlier = game();
    earlier.prepare_initial_position(3, ir::Part::PrecombatMain, 0, 0, 0, &[]).unwrap();
    earlier.set_initial_cast_history(&[]).unwrap();
    earlier.set_initial_game_cast_counts(&[Some(0), Some(9)]).unwrap();
    let once = earlier.put(0, 0, Zone::Hand);
    earlier.act(0, free(once)).unwrap();
    cite!(["118.9", "601.2i"]; assert_eq!(earlier.players[0].game_casts, Some(1)));
    decline(&mut earlier);
}
