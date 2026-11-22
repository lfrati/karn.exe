//! Full modal counter cards newly enabled by the shared suspended-card noun phrase.
use crate::support::{cite, mana_pool, oracle_read, paid};
use game::announcement::CastCost;
use game::state::{Choice, Decision};
use game::{Game, Move, ObjId, Zone};

fn setup(name: &str) -> Game {
    let defs = [name, "Rift Bolt", "Mountain", "Hill Giant"].map(oracle_read).to_vec();
    // Uncited (admission): complete pinned Oracle texts of both modal consumers.
    for def in &defs { game::play::check(def).unwrap_or_else(|why| panic!("{}: {}", def.name(), why.0)); }
    Game::new(defs, 2, 20)
}
fn passes(game: &mut Game) { for _ in 0..2 { game.act(game.priority, Move::Pass).unwrap(); } }
fn suspended(game: &mut Game, count: u32) -> ObjId {
    let card = game.put(0, 1, Zone::Exile); game.put_counters(card, "time", count); card
}
fn sand_mage(game: &mut Game) {
    let card = game.put(0, 0, Zone::Hand); game.players[0].pool = mana_pool("CCRR");
    game.act(0, paid("CCRR", Move::Cast { card, targets: vec![] })).unwrap(); passes(game);
    cite!(["603.3c", "700.2"]; assert!(matches!(game.decision, Some(Decision::Modes { player: 0, .. }))));
}
fn target(mode: usize, card: ObjId) -> Move { Move::Modes(vec![mode], Box::new(Move::ChooseTargets(vec![vec![card.into()]]))) }

#[test]
fn r702_62a_shivan_sand_mage_removes_the_last_actual_time_counter_and_offers_the_real_spell() {
    let mut game = setup("Shivan Sand-Mage"); let bolt = suspended(&mut game, 1); sand_mage(&mut game);
    cite!(["603.3c", "603.3d"]; ok: game.act(0, target(0, bolt))); passes(&mut game);
    cite!(["122.1", "702.62a"]; assert_eq!(game.counters(bolt).get("time"), None));
    cite!(["702.62a", "603.3"]; assert_eq!(game.stack.len(), 1)); passes(&mut game);
    cite!(["702.62a"]; assert!(matches!(&game.decision, Some(Decision::Choose { player: 0, choice: Choice::Cast { cards } }) if cards == &[bolt])));
    game.act(0, Move::Cast { card: bolt, targets: vec![vec![game::Target::Player(1)]] }).unwrap(); passes(&mut game);
    // Uncited (full Oracle card text): the chosen Rift Bolt deals its three damage.
    assert_eq!(game.players[1].life, 17);
}

#[test]
fn r115_9b_shivan_sand_mage_counter_mode_preserves_each_target_arms_counter_requirement() {
    let mut game = setup("Shivan Sand-Mage"); let bolt = suspended(&mut game, 1);
    let no_counter = game.put(0, 1, Zone::Exile); let permanent = game.put(0, 3, Zone::Battlefield); sand_mage(&mut game);
    cite!(["115.9b", "603.3d"]; refused("target"): game.act(0, target(1, permanent)));
    cite!(["702.62b", "603.3d"]; refused("target"): game.act(0, target(1, no_counter)));
    let mut permanent_branch = game.clone();
    cite!(["115.9b", "603.3d"]; ok: game.act(0, target(1, bolt))); passes(&mut game);
    cite!(["122.1", "702.62b"]; assert_eq!(game.counters(bolt).get("time"), Some(&3)));
    permanent_branch.put_counters(permanent, "time", 1);
    cite!(["115.9b", "603.3d"]; ok: permanent_branch.act(0, target(1, permanent))); passes(&mut permanent_branch);
    cite!(["122.1"]; assert_eq!(permanent_branch.counters(permanent).get("time"), Some(&3)));
}

#[test]
fn r702_37e_timebender_morph_action_then_its_modal_trigger_removes_two_time_counters() {
    let mut game = setup("Timebender"); let bolt = suspended(&mut game, 2);
    let card = game.put(0, 0, Zone::Hand); game.players[0].pool = mana_pool("CCC");
    cite!(["702.37a"]; ok: game.act(0, paid("CCC", Move::CastFor(CastCost::Alternative(0), Box::new(Move::Cast { card, targets: vec![] })))));
    passes(&mut game); let permanent = game.battlefield[0]; game.players[0].pool = mana_pool("U");
    cite!(["702.37e"]; ok: game.act(0, paid("U", Move::TurnFaceUp(permanent, game::face::Way::Morph))));
    cite!(["603.3c", "700.2"]; assert!(matches!(game.decision, Some(Decision::Modes { player: 0, .. }))));
    cite!(["603.3d"]; ok: game.act(0, target(0, bolt))); passes(&mut game);
    cite!(["122.1", "702.62a"]; assert_eq!(game.counters(bolt).get("time"), None));
    cite!(["702.62a", "603.3"]; assert_eq!(game.stack.len(), 1));
}
