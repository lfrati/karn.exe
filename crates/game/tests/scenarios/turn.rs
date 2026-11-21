//! The turn (CR 500–514), rule by rule: the parts of a turn in order, who has priority in each,
//! the turn-based actions (703.4), and what may be done when.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::play::check;
use game::state::Decision;
use game::{Event, Game, Move, Target, Zone};
use ir::Part;

const MOUNTAIN: usize = 0;
const BEARS: usize = 1;
const BOLT: usize = 2;
const RAM: usize = 3;
const FORCE: usize = 4;
const DJINN: usize = 5;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None),
        card(
            "Nyx-Fleece Ram",
            "{1}{W}",
            "Enchantment Creature — Sheep",
            "At the beginning of your upkeep, you gain 1 life.",
            Some((0, 5)),
        ),
        card("Celestial Force", "{5}{W}{W}{W}", "Creature — Elemental", "At the beginning of each upkeep, you gain 3 life.", Some((7, 7))),
        card(
            "Juzám Djinn",
            "{2}{B}{B}",
            "Creature — Djinn",
            "At the beginning of your upkeep, this creature deals 1 damage to you.",
            Some((5, 5)),
        ),
    ]
}

/// A two-player game set up in player 0's first precombat main phase.
fn game() -> Game {
    Game::new(defs(), 2, 20)
}

/// The same, each player with cards to draw: a player who draws from an empty library loses
/// (704.5b), so a game that goes past a draw step needs them.
fn game_with_libraries() -> Game {
    let mut g = game();
    for player in 0..2 {
        for _ in 0..10 {
            g.put(player, MOUNTAIN, Zone::Library);
        }
    }
    g
}

/// The game goes on by the least a player can do: whoever has priority passes, and a player
/// asked to declare attackers or blockers declares none.
fn go(g: &mut Game) {
    match g.decision.clone() {
        Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
        Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
        _ => g.act(g.priority, Move::Pass).unwrap(),
    }
}

/// All players pass in succession once.
fn all_pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

/// Pass until the turn is at `part`.
fn pass_to(g: &mut Game, part: Part) {
    for _ in 0..200 {
        if g.part == part && g.decision.is_none() {
            return;
        }
        go(g);
    }
    panic!("the turn never reached {part:?}");
}

fn began(g: &Game) -> Vec<Part> {
    g.log
        .iter()
        .filter_map(|e| match e {
            Event::Began { part } => Some(*part),
            _ => None,
        })
        .collect()
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r500_2_a_phase_ends_when_all_players_pass_with_the_stack_empty() {
    let mut g = game();
    assert_eq!(g.part, Part::PrecombatMain);
    g.act(0, Move::Pass).unwrap();
    cite!(["500.2"]; assert_eq!(g.part, Part::PrecombatMain, "one player passing does not end it"));
    g.act(1, Move::Pass).unwrap();
    cite!(["500.2"]; assert_eq!(g.part, Part::BeginningOfCombat));
    cite!(["117.3a"]; assert_eq!(g.priority, 0, "the active player receives priority as a step begins (117.3a)"));
    assert_eq!(g.turn, 1);
}

#[test]
fn r500_1_the_parts_of_a_turn_come_in_order() {
    let mut g = game_with_libraries();
    pass_to(&mut g, Part::PrecombatMain);
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    // through the rest of turn 1 and all of turn 2
    while g.turn < 3 {
        go(&mut g);
    }
    let second_turn: Vec<Part> = {
        let all = began(&g);
        let from = all.iter().position(|p| *p == Part::Untap).unwrap();
        let to = all.iter().rposition(|p| *p == Part::Untap).unwrap();
        all[from..to].to_vec()
    };
    // no creature attacked, so the declare blockers and combat damage steps are skipped (508.8)
    cite!(["508.8"]; assert_eq!(
        second_turn,
        [
            Part::Untap,
            Part::Upkeep,
            Part::Draw,
            Part::PrecombatMain,
            Part::BeginningOfCombat,
            Part::DeclareAttackers,
            Part::EndOfCombat,
            Part::PostcombatMain,
            Part::End,
            Part::Cleanup,
        ]
    ));
}

#[test]
fn r502_3_the_active_player_untaps_and_r502_4_no_player_has_priority_in_the_untap_step() {
    let mut g = game();
    let mine = g.put(1, MOUNTAIN, Zone::Battlefield);
    let theirs = g.put(0, MOUNTAIN, Zone::Battlefield);
    g.tap(mine);
    g.tap(theirs);
    while g.turn == 1 {
        go(&mut g);
    }
    assert_eq!(g.active, 1);
    cite!(["500.3"]; assert_eq!(g.part, Part::Upkeep, "the untap step ended with its actions (500.3)"));
    cite!(["503.1"]; assert_eq!(g.priority, 1, "the active player has priority in the upkeep step (503.1)"));
    cite!(["502.3"]; assert!(!g.obj(mine).tapped));
    cite!(["502.3"]; assert!(g.obj(theirs).tapped, "only the active player's permanents untap"));
}

#[test]
fn r504_1_the_active_player_draws_in_the_draw_step() {
    let mut g = game();
    g.put(1, BEARS, Zone::Library);
    while g.turn == 1 {
        go(&mut g);
    }
    assert_eq!(g.players[1].hand.len(), 0, "not before the draw step");
    all_pass(&mut g);
    assert_eq!(g.part, Part::Draw);
    cite!(["504.1"]; assert_eq!(g.players[1].hand.len(), 1));
    cite!(["504.2"]; assert_eq!(g.priority, 1, "then the active player gets priority (504.2)"));
}

#[test]
fn r103_8a_the_player_who_plays_first_skips_the_draw_step_of_their_first_turn() {
    let mut g = game();
    g.put(0, BEARS, Zone::Library);
    g.begin();
    assert_eq!(g.part, Part::Upkeep);
    all_pass(&mut g);
    assert_eq!(g.part, Part::PrecombatMain);
    cite!(["103.8a"]; assert!(g.players[0].hand.is_empty()));
    cite!(["103.8a"]; assert!(!began(&g).contains(&Part::Draw)));
}

#[test]
fn r500_5_unspent_mana_empties_as_a_step_or_phase_ends() {
    let mut g = game();
    let land = g.put(0, MOUNTAIN, Zone::Battlefield);
    g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    assert_eq!(g.players[0].pool.total(), 1);
    g.act(0, Move::Pass).unwrap();
    cite!(["106.4"]; assert_eq!(g.players[0].pool.total(), 1, "it stays while the phase lasts"));
    g.act(1, Move::Pass).unwrap();
    assert_eq!(g.part, Part::BeginningOfCombat);
    cite!(["500.5"]; assert_eq!(g.players[0].pool.total(), 0));
}

#[test]
fn r117_1a_an_instant_any_time_with_priority_and_anything_else_in_a_main_phase() {
    let mut g = game();
    let (forest, island) = (g.put(0, MOUNTAIN, Zone::Battlefield), g.put(0, MOUNTAIN, Zone::Battlefield));
    let bears = g.put(0, BEARS, Zone::Hand);
    let bolt = g.put(0, BOLT, Zone::Hand);
    all_pass(&mut g);
    assert_eq!(g.part, Part::BeginningOfCombat);
    g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    g.act(0, Move::Activate { source: island, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    // Fund the Bears' green requirement too, so its refusal isolates timing.
    g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::Green));
    cite!(["117.1a"]; refused("that is cast in a main phase of your own turn with the stack empty (117.1a)"): g.act(0, paid("RG", Move::Cast { card: bears, targets: vec![] })));
    cite!(["117.1a"]; ok: g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] })));
    all_pass(&mut g);
    assert_eq!(g.players[1].life, 17);
    cite!(["117.3b"]; assert_eq!(g.priority, 0, "the active player receives priority after it resolves (117.3b)"));
}

#[test]
fn r305_1_a_land_is_played_in_a_main_phase_of_ones_own_turn_and_r305_2_one_each_turn() {
    let mut g = game_with_libraries();
    let (a, b, c) = (g.put(0, MOUNTAIN, Zone::Hand), g.put(0, MOUNTAIN, Zone::Hand), g.put(1, MOUNTAIN, Zone::Hand));
    crate::support::refused(g.act(1, Move::PlayLand(c)), "that player does not have priority");
    all_pass(&mut g);
    cite!(["305.1"]; refused("a land is played in a main phase of your own turn with the stack empty (305.1)"): g.act(0, Move::PlayLand(a)));
    pass_to(&mut g, Part::PostcombatMain);
    cite!(["305.1"]; ok: g.act(0, Move::PlayLand(a)));
    cite!(["305.2"]; refused("you have played a land this turn (305.2)"): g.act(0, Move::PlayLand(b)));
    // the other player's turn: theirs to play one, not ours
    while g.turn == 1 {
        go(&mut g);
    }
    pass_to(&mut g, Part::PrecombatMain);
    cite!(["305.1"]; ok: g.act(1, Move::PlayLand(c)));
    // and in our next turn we may again
    while g.turn == 2 {
        go(&mut g);
    }
    pass_to(&mut g, Part::PrecombatMain);
    cite!(["305.2"]; ok: g.act(0, Move::PlayLand(b)));
}

#[test]
fn r514_1_the_active_player_discards_down_to_seven_cards() {
    let mut g = game();
    let hand: Vec<_> = (0..9).map(|_| g.put(0, MOUNTAIN, Zone::Hand)).collect();
    pass_to(&mut g, Part::End);
    all_pass(&mut g);
    assert_eq!(g.part, Part::Cleanup);
    cite!(["514.1"]; assert_eq!(g.decision, Some(Decision::Discard { player: 0, count: 2 })));
    // no player has priority in the cleanup step (514.3)
    cite!(["514.3"]; refused("a decision is pending: discard down to your maximum hand size (514.1)"): g.act(0, Move::Pass));
    cite!(["514.1"]; refused("discard exactly 2 card(s) (514.1)"): g.act(0, Move::Discard(vec![hand[0]])));
    cite!(["514.1"]; refused("that decision is another player's"): g.act(1, Move::Discard(vec![hand[0], hand[1]])));
    g.act(0, Move::Discard(vec![hand[0], hand[1]])).unwrap();
    cite!(["404.3"]; ok: g.act(0,Move::Choose(game::state::Answer::Objects(vec![hand[0],hand[1]]))));
    cite!(["514.1"]; assert_eq!(g.players[0].hand.len(), 7));
    assert_eq!(g.players[0].graveyard.len(), 2);
    assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Discarded { player: 0, .. })).count(), 2);
    assert_eq!((g.turn, g.active, g.part), (2, 1, Part::Upkeep), "then the next turn begins");
}

#[test]
fn r500_6_an_ability_triggers_at_the_beginning_of_the_step_its_text_names() {
    let mut g = game_with_libraries();
    g.put(0, RAM, Zone::Battlefield);
    g.put(0, FORCE, Zone::Battlefield);
    g.put(1, DJINN, Zone::Battlefield);
    // player 1's turn: their upkeep is not "your upkeep" for the Ram; it is one of "each
    // upkeep" for the Force, and "your upkeep" for their Djinn
    while g.turn == 1 {
        go(&mut g);
    }
    assert_eq!(g.part, Part::Upkeep);
    cite!(["503.1a"]; assert_eq!(g.stack.len(), 2, "on the stack before the active player gets priority (503.1a)"));
    all_pass(&mut g);
    all_pass(&mut g);
    cite!(["500.6"]; assert_eq!(g.players[0].life, 23));
    cite!(["500.6"]; assert_eq!(g.players[1].life, 19));
    assert_eq!(g.part, Part::Upkeep, "the step goes on until all pass with the stack empty");
    // player 0's next turn: the Ram and the Force both trigger
    while g.turn == 2 {
        go(&mut g);
    }
    // both are player 0's, who puts them on the stack in the order they choose (603.3b)
    cite!(["603.3b"]; assert!(matches!(g.decision, Some(Decision::Order { player: 0, .. }))));
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    assert_eq!(g.stack.len(), 2);
    all_pass(&mut g);
    all_pass(&mut g);
    cite!(["500.6"]; assert_eq!(g.players[0].life, 27));
    assert_eq!(g.players[1].life, 19);
}
