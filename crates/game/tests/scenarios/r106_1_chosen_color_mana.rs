//! A color selected on entry is the linked value used by its mana ability (607.2d).
use crate::support::{cite, oracle, mana_pool, paid};
use game::{Game, Move, Zone};
use game::state::Answer;

#[test]
fn r607_2d_coldsteel_heart_mana_reads_its_own_entry_choice() {
    let mut g = Game::new(vec![oracle("Coldsteel Heart"), oracle("Island")], 2, 20);
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool = mana_pool("CC");
    g.act(0, paid("CC", Move::Cast { card, targets: vec![] })).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    let mut fork = g.clone();
    for (g, color) in [(&mut g, ir::Color::Red), (&mut fork, ir::Color::Blue)] {
        g.act(0, Move::Choose(Answer::Value(game::binding::Value::Color(color)))).unwrap();
        let heart = g.battlefield[0];
        cite!(["614.12"]; assert!(g.obj(heart).tapped));
        // Untapping is fixture setup; the tested action is the mana activation.
        g.set_tapped(heart, false);
        g.act(0, Move::Activate { source: heart, ability: 2, targets: vec![], paying: vec![] }).unwrap();
        cite!(["106.1a", "607.2d"]; assert_eq!(g.players[0].pool.total(), 1));
        cite!(["106.1a", "607.2d"]; assert_eq!(g.players[0].pool.colors[color as usize], 1));
        cite!(["605.3b"]; assert!(g.stack.is_empty() && g.decision.is_none()));
    }
}

#[test]
fn r607_2d_utopia_sprawl_adds_its_color_to_the_enchanted_forests_controller() {
    let mut g = Game::new(vec![oracle("Utopia Sprawl"), oracle("Forest")], 2, 20);
    let forest = g.put(1, 1, Zone::Battlefield);
    let aura = g.put(0, 0, Zone::Hand);
    g.players[0].pool = mana_pool("G");
    g.act(0, paid("G", Move::Cast { card: aura, targets: vec![vec![game::Target::Object(forest)]] })).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Choose(Answer::Value(game::binding::Value::Color(ir::Color::Red)))).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["106.12a", "607.2d"]; assert_eq!(g.players[1].pool.colors[ir::Color::Green as usize], 1));
    cite!(["106.12a", "607.2d"]; assert_eq!(g.players[1].pool.colors[ir::Color::Red as usize], 1));
    cite!(["106.12a"]; assert_eq!(g.players[0].pool.total(), 0));
    cite!(["605.4a"]; assert!(g.stack.is_empty() && g.decision.is_none()));
}
