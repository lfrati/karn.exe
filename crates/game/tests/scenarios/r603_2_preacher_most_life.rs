//! Preacher's two life conditions are checked as it attacks, independently and including ties.
use crate::support::{cite, mana_pool, oracle, paid};
use game::{Game, Move, ObjId, Target, Zone};
use game::state::Decision;

fn game(life: [i32; 3]) -> (Game, ObjId) {
    let mut g = Game::new(vec![oracle("Preacher of the Schism"), oracle("Forest"),
        oracle("Lightning Bolt"), oracle("Jace Beleren")], 3, 20);
    for (player, life) in life.into_iter().enumerate() {
        g.players[player].life = life;
        for _ in 0..8 { g.put(player, 1, Zone::Library); }
    }
    let preacher = g.put(0, 0, Zone::Battlefield);
    (g, preacher)
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() - g.players.iter().filter(|p| p.lost).count() { g.act(g.priority, Move::Pass).unwrap(); } }
fn attack(g: &mut Game, preacher: ObjId, defender: Target) {
    for _ in 0..100 {
        if g.decision == Some(Decision::Attackers { player: 0 }) { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(0, Move::Attack(vec![(preacher, defender)])).unwrap();
    if let Some(Decision::Order { player, abilities }) = &g.decision {
        let (player, count) = (*player, abilities.len());
        g.act(player, Move::Order((0..count).collect())).unwrap();
    }
}
fn tokens(g: &Game) -> usize { g.battlefield.iter().filter(|id| g.obj(**id).token()).count() }
fn resolve(g: &mut Game) { while !g.stack.is_empty() { pass(g); } }

#[test]
fn r603_2_preacher_checks_each_player_and_each_tie_at_attack_time() {
    for (life, token, draw) in [([20,30,10],1,0), ([30,20,10],0,1), ([20,20,10],1,1),
        ([20,20,30],0,0), ([20,10,20],0,1), ([10,20,20],1,0)] {
        let (mut g, preacher) = game(life);
        attack(&mut g, preacher, Target::Player(1));
        cite!(["603.2", "603.3b"]; case:format!("{life:?}"); assert_eq!(g.stack.len(), token+draw));
        resolve(&mut g);
        cite!(["608.2c"]; assert_eq!(tokens(&g), token));
        cite!(["608.2c", "121.2"]; assert_eq!(g.players[0].hand.len(), draw));
        cite!(["608.2c", "119.3"]; assert_eq!(g.players[0].life, life[0]-draw as i32));
        for token in g.battlefield.iter().copied().filter(|id| g.obj(*id).token()) {
            cite!(["111.3"]; assert_eq!((g.power(token),g.toughness(token)), (Some(1),Some(1))));
            cite!(["111.3", "702.15a"]; assert!(g.has(token,"Lifelink")));
        }
    }
}

#[test]
fn r508_1b_attacking_a_leaders_planeswalker_does_not_attack_that_player() {
    let (mut g, preacher) = game([20,30,10]);
    let jace = g.put(1, 3, Zone::Battlefield);
    g.set_initial_counters(jace, "loyalty", 3);
    attack(&mut g, preacher, Target::Object(jace));
    cite!(["508.1b", "603.2"]; assert!(g.stack.is_empty()));
    cite!(["508.1b"]; assert_eq!(tokens(&g), 0));
}

#[test]
fn r603_2_while_is_not_an_intervening_if_and_new_life_totals_do_not_cancel_either_trigger() {
    let (mut g, preacher) = game([20,20,19]);
    attack(&mut g, preacher, Target::Player(1));
    cite!(["603.2", "603.3b"]; assert_eq!(g.stack.len(), 2));
    // Both players who satisfied their distinct trigger condition fall below the
    // third player before either trigger resolves. These are actual response spells.
    for player in [0,1] {
        let bolt = g.put(0, 2, Zone::Hand);
        g.players[0].pool = mana_pool("R");
        g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Player(player)]] })).unwrap();
        pass(&mut g);
    }
    cite!(["120.3a"]; assert_eq!((g.players[0].life,g.players[1].life,g.players[2].life), (17,17,19)));
    let branch = g.clone();
    for mut g in [g, branch] {
        resolve(&mut g);
        // Pinned Preacher rulings 2023-11-10 explicitly keep both already-triggered abilities.
        cite!(["603.2", "603.4", "608.2a"]; assert_eq!((tokens(&g),g.players[0].hand.len(),g.players[0].life), (1,1,16)));
    }
}

#[test]
fn r800_4a_a_departed_players_higher_total_no_longer_prevents_either_trigger() {
    let (mut g, preacher) = game([20,20,30]);
    g.act(2, Move::Concede).unwrap();
    attack(&mut g, preacher, Target::Player(1));
    cite!(["800.4a", "603.2"]; assert_eq!(g.stack.len(), 2));
    resolve(&mut g);
    cite!(["800.4a", "608.2c"]; assert_eq!((tokens(&g),g.players[0].hand.len()), (1,1)));
}
