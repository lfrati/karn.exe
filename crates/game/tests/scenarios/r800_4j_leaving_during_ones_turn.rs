//! A player who loses a multiplayer game during their own turn leaves it, and the turn goes
//! on to its end without an active player (800.4a, 800.4j); the next turn is the next player's.
use crate::support::{cite, mana_pool, oracle};
use game::state::Decision;
use game::{Game, Move, Target, Zone};
use ir::Part;

fn step(g: &mut Game) {
    match g.decision.clone() {
        Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
        Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
        _ => g.act(g.priority, Move::Pass).unwrap(),
    };
}

// Rules: 104.3e, 800.4a, 800.4j, 800.4k
// Given: three players; player 0 controls Door to Nothingness; in player 1's precombat main
// phase, players 1 and 2 pass and player 0 activates it targeting player 1.
// When: it resolves.
// Expect: player 1 loses and leaves; the game goes on; the turn ends without them and the
// next turns are player 2's, then player 0's.
#[test]
fn r800_4j_the_active_player_who_loses_leaves_and_the_turn_goes_on() {
    // Door to Nothingness: "{W}{W}{U}{U}{B}{B}{R}{R}{G}{G}, {T}, Sacrifice this artifact:
    // Target player loses the game."
    let mut g = Game::new(vec![oracle("Door to Nothingness"), oracle("Grizzly Bears")], 3, 20);
    for p in 0..3 { for _ in 0..5 { g.put(p, 1, Zone::Library); } }
    let door = g.put(0, 0, Zone::Battlefield);
    let bears = g.put(1, 1, Zone::Battlefield);
    while !(g.active == 1 && g.part == Part::PrecombatMain) { step(&mut g); }
    let turn = g.turn;
    g.act(1, Move::Pass).unwrap();
    g.act(2, Move::Pass).unwrap();
    let ability = g.abilities(door).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).unwrap();
    let mana = "WWUUBBRRGG";
    g.players[0].pool = mana_pool(mana);
    g.act(0, Move::Paying(mana_pool(mana), Box::new(Move::Activate { source: door, ability, targets: vec![vec![Target::Player(1)]], paying: vec![] }))).unwrap();
    while !g.stack.is_empty() { step(&mut g); }
    cite!(["104.3e", "800.4a"]; assert!(g.players[1].lost && !g.finished()));
    cite!(["800.4a"]; assert_eq!(g.obj(bears).zone, Zone::Gone));
    // the turn goes on without an active player (800.4j)
    cite!(["800.4j"]; assert_eq!((g.turn, g.part), (turn, Part::PrecombatMain)));
    while g.turn == turn { step(&mut g); }
    cite!(["800.4k", "500.1"]; assert_eq!(g.active, 2));
    while g.turn == turn + 1 { step(&mut g); }
    cite!(["800.4k"]; assert_eq!(g.active, 0));
}
