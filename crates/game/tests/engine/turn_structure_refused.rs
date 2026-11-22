//! What the grammar reads of the turn's structure but the engine does not run stays refused, and
//! the refusal names what is not run. The next step or combat phase of a player's is skipped
//! (614.10) and combat and main phases are added (500.8) through the turn plan;
//! every combat phase of a turn, a step other than the next one and added ending phases
//! (which need a cleanup continuation) are not. Beginning phases run outside the starting
//! player's first two-player turn, whose draw omission remains explicitly refused.
//! Extra turns run through the shared turn scheduler (500.7). Plain next-turn skips now run before turn-begin actions; their
//! single/count-two execution and boundaries are tested in r500_7_created_turn_rules.
use game::CardDef;

fn refusal(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> String {
    let d = CardDef::new(name, cost, types, text, pt).expect(name);
    match game::play::check(&d) {
        Ok(()) => panic!("{name} should be refused"),
        Err(u) => u.0,
    }
}

#[test]
fn r614_10_skipping_a_step_that_is_not_the_next_one_is_refused() {
    let why = refusal(
        "Elfhame Sanctuary",
        "{1}{G}",
        "Enchantment",
        "At the beginning of your upkeep, you may search your library for a basic land card, reveal that card, put it into your hand, then shuffle. If you do, you skip your draw step this turn.",
        None,
    );
    assert!(why.contains("not the next one (614.10)"), "{why}");
}

#[test]
fn r614_10a_skipping_all_combat_phases_of_a_next_turn_is_refused() {
    let why = refusal("False Peace", "{W}", "Sorcery", "Target player skips all combat phases of their next turn.\nDraw a card.", None);
    assert!(why.contains("614.10"), "{why}");
}

#[test]
fn r500_8_an_additional_beginning_phase_compiles() {
    // The shared plan runs combat/main and complete beginning phases (500.8).
    let assault = CardDef::new("Relentless Assault", "{2}{R}{R}", "Sorcery",
        "Untap all creatures that attacked this turn. After this main phase, there is an additional combat phase followed by an additional main phase.", None).unwrap();
    game::play::check(&assault).unwrap();
    let sphinx = CardDef::new("Sphinx of the Second Sun", "{6}{U}{U}", "Creature — Sphinx",
        "Flying\nAt the beginning of each of your postcombat main phases, there is an additional beginning phase after this phase.", Some((6, 6))).unwrap();
    game::play::check(&sphinx).unwrap();
}

#[test]
fn r500_7_time_stretch_takes_two_extra_turns_then_restores_normal_order() {
    use game::{Game, Move, Target, Zone};
    let d = CardDef::new("Time Stretch", "{8}{U}{U}", "Sorcery", "Target player takes two extra turns after this one.", None).unwrap();
    game::play::check(&d).unwrap();
    let filler = CardDef::new("Land", "", "Land", "", None).unwrap();
    let mut g = Game::new(vec![d, filler], 3, 20);
    for p in 0..3 { for _ in 0..5 { g.put(p, 1, Zone::Library); } }
    let spell = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[1] = 10;
    g.act(0, crate::support::paid("UUUUUUUUUU", Move::Cast { card: spell, targets: vec![vec![Target::Player(2)]] })).unwrap();
    for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); }
    let mut copy = g.clone();
    for g in [&mut g, &mut copy] {
        for active in [2, 2, 1] {
            let turn = g.turn;
            while g.turn == turn { g.act(g.priority, Move::Pass).unwrap(); }
            assert_eq!(g.active, active);
        }
    }
}
