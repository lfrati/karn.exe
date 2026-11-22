//! A source-scoped timing permission changes timing, retaining the per-permanent use limit.
use crate::support::{cite, mana_pool, oracle, paid};
use game::{Game, Move, ObjId, Target, Zone};

fn activate(g: &mut Game, source: ObjId, ability: usize, targets: Vec<Vec<Target>>) -> Result<(), String> {
    g.act(0, Move::Activate { source, ability, targets, paying: vec![] })
}
fn pass(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn next_main(g: &mut Game) {
    let turn = g.turn;
    for _ in 0..200 {
        if g.turn != turn && g.part == ir::Part::PrecombatMain && g.stack.is_empty() && g.priority == 0 { return; }
        let action = match g.decision {
            Some(game::state::Decision::Attackers { .. }) => Move::Attack(vec![]),
            Some(game::state::Decision::Blockers { .. }) => Move::Block(vec![]),
            _ => Move::Pass,
        };
        g.act(g.priority, action).unwrap();
    }
    panic!("no next main");
}

#[test]
fn r606_3_emperor_responds_on_entry_turn_but_only_with_her_own_once_per_turn_ability() {
    let mut g = Game::new(["The Wandering Emperor", "Jace Beleren", "Grizzly Bears", "Lightning Bolt", "Forest"].into_iter().map(oracle).collect::<Vec<_>>(), 2, 20);
    for player in 0..2 { for _ in 0..8 { g.put(player, 4, Zone::Library); } }
    // Uncited (fixture): begin in the opponent's main phase with our priority.
    g.active = 1;
    g.turn = 2;
    let jace = g.put(0, 1, Zone::Battlefield);
    g.set_initial_counters(jace, "loyalty", 3);
    let bear = g.put(0, 2, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool = mana_pool("WWWW");
    cite!(["702.8a"]; ok: g.act(0, paid("WWWW", Move::Cast { card, targets: vec![] })));
    pass(&mut g);
    let emperor = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(0)).unwrap();
    let bolt = g.put(1, 3, Zone::Hand);
    g.players[1].pool = mana_pool("R");
    g.act(1, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Player(0)]] })).unwrap();
    g.act(1, Move::Pass).unwrap();
    cite!(["606.3"]; refused("in a main phase of your own turn"): activate(&mut g, jace, 0, vec![]));
    // Uncited (Oracle): her static ability permits instant timing on her entry turn.
    activate(&mut g, emperor, 2, vec![vec![Target::Object(bear)]]).unwrap();
    cite!(["606.4"]; assert_eq!(g.obj(emperor).counters.get("loyalty"), Some(&4)));
    pass(&mut g);
    cite!(["122.1a"]; assert_eq!(g.obj(bear).counters.get("+1/+1"), Some(&1)));
    pass(&mut g);
    g.act(1, Move::Pass).unwrap();
    cite!(["606.3"]; refused("already been activated this turn"): activate(&mut g, emperor, 3, vec![]));
    next_main(&mut g); // Our next turn: normal sorcery timing still works.
    cite!(["606.3"]; ok: activate(&mut g, emperor, 3, vec![]));
    pass(&mut g);
    next_main(&mut g); // The next opponent turn: the old entry no longer grants permission.
    cite!(["606.3", "608.2i"]; refused("in a main phase of your own turn"): activate(&mut g, emperor, 3, vec![]));
}
