//! Priority and what happens around it, rule by rule: who receives priority and when (117.3–
//! 117.5), the order in which players act together (101.4), the steps in which no player
//! receives priority (500.3, 502.4, 514.3), the turn-based actions (703.4), how triggered
//! abilities wait for a player to receive priority (503.1a, 603.3), state-based actions
//! (704.5), and a player who leaves the game (800.4a).

use crate::support::{cite, paid};
use game::card::CardDef;
use game::play::check;
use game::state::{Answer, Choice, Decision};
use game::{Event, Game, Move, ObjId, Target, Zone};
use ir::{Part, Verb};

const MOUNTAIN: usize = 0;
const SWAMP: usize = 1;
const FOREST: usize = 2;
const BEARS: usize = 3;
const BOLT: usize = 4;
const SHOCK: usize = 5;
const THALLID: usize = 6;
const WARDEN: usize = 7;
const RAM: usize = 8;
const FORCE: usize = 9;
const SUN_GUIDE: usize = 10;
const GROWTH: usize = 11;
const INNOCENT_BLOOD: usize = 12;
const RATS: usize = 13;
const WURM: usize = 14;
const EMBERS: usize = 15;

fn card(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    CardDef::new(name, cost, types, text, pt).expect(name)
}

fn defs() -> Vec<CardDef> {
    vec![
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Swamp", "", "Basic Land — Swamp", "({T}: Add {B}.)", None),
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None),
        card("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None),
        card(
            "Tukatongue Thallid",
            "{G}",
            "Creature — Fungus",
            "When this creature dies, create a 1/1 green Saproling creature token.",
            Some((1, 1)),
        ),
        card("Soul Warden", "{W}", "Creature — Human Cleric", "Whenever another creature enters, you gain 1 life.", Some((1, 1))),
        card(
            "Nyx-Fleece Ram",
            "{1}{W}",
            "Enchantment Creature — Sheep",
            "At the beginning of your upkeep, you gain 1 life.",
            Some((0, 5)),
        ),
        card("Celestial Force", "{5}{W}{W}{W}", "Creature — Elemental", "At the beginning of each upkeep, you gain 3 life.", Some((7, 7))),
        card(
            "Oreskos Sun Guide",
            "{1}{W}",
            "Creature — Cat Monk",
            "Inspired — Whenever this creature becomes untapped, you gain 2 life.",
            Some((2, 2)),
        ),
        card("Giant Growth", "{G}", "Instant", "Target creature gets +3/+3 until end of turn.", None),
        card("Innocent Blood", "{B}", "Sorcery", "Each player sacrifices a creature of their choice.", None),
        card(
            "Typhoid Rats",
            "{B}",
            "Creature — Rat",
            "Deathtouch (Any amount of damage this deals to a creature is enough to destroy it.)",
            Some((1, 1)),
        ),
        card("Craw Wurm", "{4}{G}{G}", "Creature — Wurm", "", Some((6, 4))),
        card("Rain of Embers", "{1}{R}", "Sorcery", "Rain of Embers deals 1 damage to each creature and each player.", None),
    ]
}

/// An exact initial main-phase position, with empty libraries unless the test supplies cards.
fn game_of(players: usize) -> Game {
    Game::new(defs(), players, 20)
}

fn game() -> Game {
    game_of(2)
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

/// All players still in the game pass in succession once.
fn all_pass(g: &mut Game) {
    for _ in 0..g.players.iter().filter(|p| !p.lost).count() {
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

/// Pass until the next turn has begun and is at `part`.
fn next_turn_to(g: &mut Game, part: Part) {
    let turn = g.turn;
    for _ in 0..200 {
        if g.turn != turn { pass_to(g, part); return; }
        go(g);
    }
    panic!("the next turn never began: {:?}", g.asked());
}

/// Put lands onto the battlefield for a player and tap them for mana.
fn mana(g: &mut Game, player: usize, land: usize, n: usize) {
    for _ in 0..n {
        let l = g.put(player, land, Zone::Battlefield);
        g.act(player, Move::Activate { source: l, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
}

/// The parts of the turn that have begun, in order.
fn began(g: &Game) -> Vec<Part> {
    g.log
        .iter()
        .filter_map(|e| match e {
            Event::Began { part } => Some(*part),
            _ => None,
        })
        .collect()
}

/// The controllers of the objects on the stack, from the bottom.
fn stack_controllers(g: &Game) -> Vec<usize> {
    g.stack.iter().map(|o| g.obj(*o).controller).collect()
}

fn sacrifice(player: usize, among: Vec<ObjId>) -> Option<Decision> {
    Some(Decision::Choose { player, choice: Choice::Objects { verb: Verb::Sacrifice, to: None, among, count: 1 } })
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r101_4_the_active_player_chooses_first_then_the_others_in_turn_order() {
    // "the active player … makes any choices required, then the next player in turn order …,
    // followed by the remaining nonactive players in turn order. Then the actions happen
    // simultaneously." In player 1's turn of a three-player game that is 1, 2, then 0.
    let mut g = game_of(3);
    g.put(1, MOUNTAIN, Zone::Library); // Player 1 draws this card on the next turn.
    let creatures: Vec<Vec<ObjId>> =
        (0..3).map(|p| vec![g.put(p, BEARS, Zone::Battlefield), g.put(p, WURM, Zone::Battlefield)]).collect();
    next_turn_to(&mut g, Part::PrecombatMain);
    assert_eq!((g.active, g.priority), (1, 1));
    let blood = g.put(1, INNOCENT_BLOOD, Zone::Hand);
    mana(&mut g, 1, SWAMP, 1);
    g.act(1, paid("B", Move::Cast { card: blood, targets: vec![] })).unwrap();
    all_pass(&mut g);
    for p in [1, 2, 0] {
        cite!(["101.4"]; case: format!("player {p}"); assert_eq!(g.decision, sacrifice(p, creatures[p].clone()), "player {p}'s choice"));
        cite!(["101.4"]; case: format!("player {p}"); assert!(creatures.iter().flatten().all(|c| g.battlefield.contains(c)), "nothing is sacrificed until all have chosen"));
        g.act(p, Move::Choose(Answer::Objects(vec![creatures[p][0]]))).unwrap();
    }
    for c in &creatures {
        assert!(!g.battlefield.contains(&c[0]) && g.battlefield.contains(&c[1]));
    }
}

#[test]
fn r117_3a_the_active_player_receives_priority_as_a_step_begins_after_its_actions_and_triggers() {
    let mut g = game();
    g.put(1, MOUNTAIN, Zone::Library); // Player 1 draws this card on the next turn.
    g.put(1, RAM, Zone::Battlefield);
    let mut last = g.part;
    let mut seen = Vec::new();
    for _ in 0..200 {
        if g.turn >= 3 { break; }
        go(&mut g);
        if g.part != last && g.decision.is_none() {
            last = g.part;
            seen.push(g.part);
            cite!(["117.3a"]; case: format!("turn {} {:?}", g.turn, g.part); assert_eq!(g.priority, g.active, "the active player receives priority in {:?}", g.part));
            match (g.turn, g.part) {
                // after the abilities that trigger at its beginning are on the stack
                (2, Part::Upkeep) => cite!(["117.3a", "503.1a"]; assert_eq!(stack_controllers(&g), [1])),
                // after the turn-based action of drawing a card
                (2, Part::Draw) => cite!(["117.3a", "703.4d"]; assert_eq!(g.players[1].hand.len(), 1)),
                _ => {}
            }
        }
    }
    assert_eq!(g.turn, 3, "the driver reached the next full turn");
    assert!(seen.contains(&Part::Upkeep) && seen.contains(&Part::Draw), "both observed steps actually occurred");
    assert_eq!(g.players[1].hand.len(), 1);
    cite!(["117.3a", "502.4"]; assert!(!seen.contains(&Part::Untap), "no player receives priority during the untap step"));
    cite!(["514.3"]; assert!(!seen.contains(&Part::Cleanup), "nor, usually, during the cleanup step (514.3)"));
}

#[test]
fn r117_3d_a_player_who_passes_gives_priority_to_the_next_player_in_turn_order() {
    let mut g = game_of(3);
    mana(&mut g, 0, MOUNTAIN, 1);
    g.act(0, Move::Pass).unwrap();
    cite!(["117.3d"]; assert_eq!(g.priority, 1));
    assert_eq!(g.players[0].pool.total(), 1, "the mana stays in the pool of the player who passed");
    cite!(["117.3d"]; refused("that player does not have priority"): g.act(0, Move::Pass));
    g.act(1, Move::Pass).unwrap();
    cite!(["117.3d"]; assert_eq!(g.priority, 2));
    g.act(2, Move::Pass).unwrap();
    cite!(["117.4"]; assert_eq!(g.part, Part::BeginningOfCombat, "all three passed in succession (117.4)"));
}

#[test]
fn r117_4_when_all_players_pass_in_succession_the_top_of_the_stack_resolves() {
    // "all players pass without taking any actions in between passing"
    let mut g = game();
    mana(&mut g, 0, MOUNTAIN, 1);
    let bolt = g.put(0, BOLT, Zone::Hand);
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] })).unwrap();
    g.act(0, Move::Pass).unwrap();
    // player 1 acts instead of passing: player 0's pass is no longer one of a succession
    mana(&mut g, 1, MOUNTAIN, 1);
    let shock = g.put(1, SHOCK, Zone::Hand);
    g.act(1, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Player(0)]] })).unwrap();
    g.act(1, Move::Pass).unwrap();
    cite!(["117.4"]; assert_eq!(g.stack.len(), 2, "only one player has passed since the Shock was cast"));
    assert_eq!(g.priority, 0);
    g.act(0, Move::Pass).unwrap();
    cite!(["117.4"]; assert_eq!(g.stack.len(), 1, "only the top resolves"));
    assert_eq!((g.players[0].life, g.players[1].life), (18, 20));
    all_pass(&mut g);
    assert_eq!((g.players[1].life, g.part), (17, Part::PrecombatMain));
    // with the stack empty, the phase ends
    all_pass(&mut g);
    cite!(["117.4"]; assert_eq!(g.part, Part::BeginningOfCombat));
}

#[test]
fn r117_5_state_based_actions_then_triggered_abilities_before_a_player_receives_priority() {
    let mut g = game();
    let thallid = g.put(1, THALLID, Zone::Battlefield);
    mana(&mut g, 0, MOUNTAIN, 1);
    let bolt = g.put(0, BOLT, Zone::Hand);
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Object(thallid)]] })).unwrap();
    all_pass(&mut g);
    // the Bolt resolved; before player 0 receives priority the Thallid with lethal damage was
    // destroyed (704.5g), and the ability that triggered on its dying is on the stack
    assert_eq!(g.priority, 0);
    cite!(["117.5", "704.5g"]; assert!(!g.battlefield.contains(&thallid)));
    assert_eq!(g.players[1].graveyard.len(), 1);
    cite!(["117.5", "603.3"]; assert_eq!(g.stack.len(), 1));
    assert_eq!(g.obj(g.stack[0]).ability.map(|(_, i)| i), Some(0));
    assert_eq!(stack_controllers(&g), [1]);
}

#[test]
fn r402_2_a_player_may_have_any_number_of_cards_in_hand_and_discards_only_in_their_cleanup() {
    let mut g = game();
    g.put(1, MOUNTAIN, Zone::Library); // Player 1 draws this card on the next turn.
    for _ in 0..9 {
        g.put(1, BEARS, Zone::Hand);
    }
    // player 0's cleanup step is not player 1's: they keep nine cards
    next_turn_to(&mut g, Part::Draw);
    assert_eq!(g.active, 1);
    cite!(["402.2"]; assert_eq!(g.players[1].hand.len(), 10, "nine, and the card drawn"));
    assert!(!g.log.iter().any(|e| matches!(e, Event::Discarded { .. })));
    while g.decision.is_none() {
        go(&mut g);
    }
    assert_eq!(g.part, Part::Cleanup);
    cite!(["402.2", "514.1"]; assert_eq!(g.decision, Some(Decision::Discard { player: 1, count: 3 }), "down to seven, the maximum hand size"));
}

#[test]
fn r500_3_a_step_without_priority_ends_when_its_actions_are_done() {
    // the untap step
    let mut g = game();
    g.begin();
    cite!(["500.3"]; assert_eq!(began(&g), [Part::Untap, Part::Upkeep]));
    assert_eq!(g.part, Part::Upkeep);
    // the cleanup step, with no card to discard: the turn ends without anyone passing in it
    let mut g = game();
    pass_to(&mut g, Part::End);
    let from = g.log.len();
    all_pass(&mut g);
    let parts: Vec<Part> = g.log[from..]
        .iter()
        .filter_map(|e| match e {
            Event::Began { part } => Some(*part),
            _ => None,
        })
        .collect();
    cite!(["500.3", "514.3"]; assert_eq!(parts, [Part::Cleanup, Part::Untap, Part::Upkeep]));
    assert_eq!((g.turn, g.active), (2, 1));
}

#[test]
fn r502_4_what_triggers_in_the_untap_step_waits_until_a_player_would_receive_priority() {
    let mut g = game();
    let guide = g.put(0, SUN_GUIDE, Zone::Battlefield);
    g.set_tapped(guide, true);
    g.begin();
    // it became untapped in the untap step, and its ability triggered there
    let untapped = g.log.iter().position(|e| *e == Event::Untapped { object: guide }).expect("untapped");
    let upkeep = g.log.iter().position(|e| *e == Event::Began { part: Part::Upkeep }).unwrap();
    assert!(untapped < upkeep);
    // it went on the stack only when player 0 was to receive priority, in the upkeep
    assert_eq!(g.part, Part::Upkeep);
    assert_eq!(g.priority, 0);
    cite!(["502.4", "503.1a"]; assert_eq!(g.stack.len(), 1));
    cite!(["502.4"]; assert_eq!(g.players[0].life, 20, "it could not resolve in the untap step"));
    all_pass(&mut g);
    assert_eq!(g.players[0].life, 22);
}

#[test]
fn r503_1a_what_triggered_in_the_untap_step_and_at_the_beginning_of_upkeep_is_on_the_stack_first() {
    let mut g = game();
    let guide = g.put(0, SUN_GUIDE, Zone::Battlefield);
    let ram = g.put(0, RAM, Zone::Battlefield);
    g.set_tapped(guide, true);
    g.begin();
    assert_eq!((g.part, g.priority), (Part::Upkeep, 0));
    // both are player 0's, who puts them on the stack in the order they choose (603.3b)
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    let mut sources: Vec<ObjId> = g.stack.iter().filter_map(|o| g.obj(*o).ability.and_then(|(s, _)| s)).collect();
    sources.sort();
    cite!(["503.1a"]; assert_eq!(sources, [guide, ram], "both are on the stack before the active player gets priority"));
    all_pass(&mut g);
    all_pass(&mut g);
    assert_eq!(g.players[0].life, 23);
}

#[test]
fn r505_6a_creature_spells_are_cast_by_the_active_player_in_a_main_phase() {
    let mut g = game();
    // the other player, with priority in the active player's main phase, cannot
    g.act(0, Move::Pass).unwrap();
    mana(&mut g, 1, FOREST, 2);
    let theirs = g.put(1, BEARS, Zone::Hand);
    cite!(["117.1a"]; refused("that is cast in a main phase of your own turn with the stack empty (117.1a)"): g.act(1, paid("GG", Move::Cast { card: theirs, targets: vec![] })));
    // the active player can in the precombat main phase
    let mut g = game();
    mana(&mut g, 0, FOREST, 2);
    let bears = g.put(0, BEARS, Zone::Hand);
    cite!(["505.6a"]; ok: g.act(0, paid("GG", Move::Cast { card: bears, targets: vec![] })));
    // not in the beginning of combat step, nor in the end step; again in the postcombat main
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Hand);
    for part in [Part::BeginningOfCombat, Part::End] {
        pass_to(&mut g, part);
        mana(&mut g, 0, FOREST, 2);
        cite!(["117.1a"]; case: format!("{part:?}"); refused("that is cast in a main phase of your own turn with the stack empty (117.1a)"): g.act(0, paid("GG", Move::Cast { card: bears, targets: vec![] })));
        if part == Part::BeginningOfCombat {
            pass_to(&mut g, Part::PostcombatMain);
            mana(&mut g, 0, FOREST, 2);
            let more = g.put(0, BEARS, Zone::Hand);
            cite!(["505.6a"]; ok: g.act(0, paid("GG", Move::Cast { card: more, targets: vec![] })));
            all_pass(&mut g);
        }
    }
}

#[test]
fn r514_3_no_player_receives_priority_in_the_cleanup_step() {
    let mut g = game();
    let hand: Vec<ObjId> = (0..8).map(|_| g.put(0, BEARS, Zone::Hand)).collect();
    let bolt = g.put(0, BOLT, Zone::Hand);
    let shock = g.put(1, SHOCK, Zone::Hand);
    pass_to(&mut g, Part::End);
    all_pass(&mut g);
    assert_eq!(g.part, Part::Cleanup);
    cite!(["514.1"]; assert_eq!(g.decision, Some(Decision::Discard { player: 0, count: 2 })));
    // no spell can be cast, by either player, and there is no priority to pass
    cite!(["514.3"]; refused("a decision is pending: discard down to your maximum hand size (514.1)"): g.act(0, Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] }));
    cite!(["514.3"]; refused("a decision is pending: discard down to your maximum hand size (514.1)"): g.act(1, Move::Cast { card: shock, targets: vec![vec![Target::Player(0)]] }));
    cite!(["514.3"]; refused("a decision is pending: discard down to your maximum hand size (514.1)"): g.act(0, Move::Pass));
    cite!(["514.3"]; refused("a decision is pending: discard down to your maximum hand size (514.1)"): g.act(1, Move::Pass));
    g.act(0, Move::Discard(hand[..2].to_vec())).unwrap();
    cite!(["404.3"]; ok: g.act(0, Move::Choose(game::state::Answer::Objects(hand[..2].to_vec()))));
    cite!(["500.3"]; assert_eq!((g.turn, g.part), (2, Part::Upkeep), "the step ended with its actions (500.3)"));
}

#[test]
fn r514_3a_a_state_based_action_in_the_cleanup_step_gives_priority_and_another_cleanup_step_follows() {
    // the Bears are 2/2 with two -1/-1 counters and +3/+3 from Giant Growth until end of
    // turn: as the effect ends in the cleanup step (514.2) their toughness is 0 (704.5f)
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    mana(&mut g, 0, FOREST, 1);
    let growth = g.put(0, GROWTH, Zone::Hand);
    g.act(0, paid("G", Move::Cast { card: growth, targets: vec![vec![Target::Object(bears)]] })).unwrap();
    all_pass(&mut g);
    g.put_counters(bears, "-1/-1", 2);
    assert_eq!(g.toughness(bears), Some(3));
    pass_to(&mut g, Part::End);
    all_pass(&mut g);
    // the state-based action is performed, then the active player gets priority
    cite!(["514.3a"]; assert_eq!((g.turn, g.part, g.priority), (1, Part::Cleanup, 0)));
    cite!(["514.2", "704.5f"]; assert!(!g.battlefield.contains(&bears)));
    assert_eq!(g.players[0].graveyard.len(), 2, "the Bears and the Giant Growth");
    // players may cast spells
    mana(&mut g, 0, MOUNTAIN, 1);
    let bolt = g.put(0, BOLT, Zone::Hand);
    cite!(["514.3a"]; ok: g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] })));
    all_pass(&mut g);
    assert_eq!(g.players[1].life, 17);
    assert_eq!((g.turn, g.part), (1, Part::Cleanup), "the stack is empty; players have not yet all passed");
    let from = g.log.len();
    all_pass(&mut g);
    let parts: Vec<Part> = g.log[from..]
        .iter()
        .filter_map(|e| match e {
            Event::Began { part } => Some(*part),
            _ => None,
        })
        .collect();
    cite!(["514.3a"]; assert_eq!(parts, [Part::Cleanup, Part::Untap, Part::Upkeep], "another cleanup step, then the next turn"));
}

#[test]
fn r603_3a_a_triggered_ability_is_controlled_by_its_sources_controller() {
    // Celestial Force is player 0's; it triggers at the beginning of player 1's upkeep
    let mut g = game();
    g.put(0, FORCE, Zone::Battlefield);
    next_turn_to(&mut g, Part::Upkeep);
    assert_eq!(g.active, 1);
    cite!(["603.3a"]; assert_eq!(stack_controllers(&g), [0], "not the active player's"));
    all_pass(&mut g);
    assert_eq!((g.players[0].life, g.players[1].life), (23, 20), "\"you\" is its controller");
}

#[test]
fn r603_3b_triggered_abilities_go_on_the_stack_in_apnap_order() {
    // each player's Soul Warden triggers on the Bears entering: the active player's goes on
    // the stack first, so the other player's is on top and resolves first
    let mut g = game();
    g.put(0, WARDEN, Zone::Battlefield);
    g.put(1, WARDEN, Zone::Battlefield);
    mana(&mut g, 0, FOREST, 2);
    let bears = g.put(0, BEARS, Zone::Hand);
    g.act(0, paid("GG", Move::Cast { card: bears, targets: vec![] })).unwrap();
    all_pass(&mut g);
    cite!(["603.3b"]; assert_eq!(stack_controllers(&g), [0, 1]));
    all_pass(&mut g);
    assert_eq!((g.players[0].life, g.players[1].life), (20, 21));
    all_pass(&mut g);
    assert_eq!((g.players[0].life, g.players[1].life), (21, 21));
}

#[test]
fn r603_7_an_effect_creates_a_delayed_triggered_ability_that_returns_the_card() {
    let mist = card(
        "Turn to Mist",
        "{1}{W/U}",
        "Instant",
        "Exile target creature. Return that card to the battlefield under its owner's control at the beginning of the next end step.",
        None,
    );
    check(&mist).unwrap();
    let bear=card("Bear","{0}","Creature — Bear","",Some((2,2)));
    let mut g=Game::new(vec![mist,bear],2,20);let creature=g.put(1,1,Zone::Battlefield);
    let spell=g.put(0,0,Zone::Hand);g.players[0].pool.colors[1]=2;
    g.act(0,paid("UU", Move::Cast{card:spell,targets:vec![vec![Target::Object(creature)]]})).unwrap();all_pass(&mut g);
    assert!(g.battlefield.is_empty());assert_eq!(g.exile.len(),1);
    pass_to(&mut g,Part::End);cite!(["603.7"]; assert_eq!(g.stack.len(),1));all_pass(&mut g);
    assert!(g.exile.is_empty());assert_eq!(g.battlefield.len(),1);
    let returned=g.battlefield[0];cite!(["400.7"]; assert_ne!(returned,creature));assert_eq!(g.obj(returned).controller,1);
}

#[test]
fn r703_4_the_turn_based_actions_happen_as_their_steps_begin_without_the_stack() {
    let mut g = game();
    g.put(1, MOUNTAIN, Zone::Library); // Player 1 draws this card on the next turn.
    let wurm = g.put(0, WURM, Zone::Battlefield);
    let land = g.put(1, MOUNTAIN, Zone::Battlefield);
    let shock = g.put(1, SHOCK, Zone::Hand);
    // player 1 Shocks the Wurm and taps their land for it
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    g.act(1, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Object(wurm)]] })).unwrap();
    all_pass(&mut g);
    assert_eq!(g.obj(wurm).damage, 2);
    // the cleanup step removes damage (703.4p); player 1's untap step untaps (703.4c); the
    // draw step draws (703.4d): each done by the time anyone could act, with nothing on the stack
    next_turn_to(&mut g, Part::Upkeep);
    cite!(["703.4p"]; assert_eq!(g.obj(wurm).damage, 0));
    cite!(["703.4c"]; assert!(!g.obj(land).tapped));
    assert!(g.stack.is_empty());
    all_pass(&mut g);
    assert_eq!(g.part, Part::Draw);
    cite!(["703.4d"]; assert_eq!(g.players[1].hand.len(), 1));
    assert!(g.stack.is_empty());
}

#[test]
fn r703_4q_as_each_step_ends_unspent_mana_empties_from_every_pool() {
    let mut g = game();
    all_pass(&mut g);
    assert_eq!(g.part, Part::BeginningOfCombat);
    mana(&mut g, 0, MOUNTAIN, 1);
    g.act(0, Move::Pass).unwrap();
    mana(&mut g, 1, SWAMP, 2);
    assert_eq!((g.players[0].pool.total(), g.players[1].pool.total()), (1, 2));
    // activating a mana ability is an action: player 0 passes again (117.4)
    g.act(1, Move::Pass).unwrap();
    cite!(["117.4"]; assert_eq!(g.part, Part::BeginningOfCombat));
    g.act(0, Move::Pass).unwrap();
    assert_eq!(g.part, Part::DeclareAttackers);
    cite!(["703.4q", "106.4"]; assert_eq!((g.players[0].pool.total(), g.players[1].pool.total()), (0, 0)));
}

#[test]
fn r704_5_the_state_based_actions_that_apply_are_all_performed() {
    // Rain of Embers deals 1 damage to each creature and each player: a player at 1 life
    // loses (704.5a) and a 1/1 is destroyed (704.5g) in the same check
    let mut g = game_of(3);
    let warden = g.put(1, WARDEN, Zone::Battlefield);
    let rats = g.put(0, RATS, Zone::Battlefield); // Owned by a surviving player: leaving-game cleanup cannot remove it.
    let bears = g.put(2, BEARS, Zone::Battlefield);
    g.players[1].life = 1;
    mana(&mut g, 0, MOUNTAIN, 2);
    let embers = g.put(0, EMBERS, Zone::Hand);
    g.act(0, paid("RR", Move::Cast { card: embers, targets: vec![] })).unwrap();
    all_pass(&mut g);
    assert_eq!(g.part, Part::PrecombatMain);
    assert!(g.stack.is_empty());
    cite!(["704.5a"]; assert!(g.players[1].lost));
    assert!(!g.battlefield.contains(&warden));
    cite!(["704.5g", "704.3"]; assert!(g.log.contains(&Event::Destroyed { object: rats }), "lethal-damage SBA also ran"));
    assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(RATS)));
    assert!(g.battlefield.contains(&bears), "1 damage is not lethal to a 2/2");
    assert_eq!((g.players[0].life, g.players[2].life), (19, 19));
    assert_eq!(g.priority, 0);
}

#[test]
fn r704_5h_a_creature_dealt_damage_by_a_source_with_deathtouch_is_destroyed() {
    let mut g = game();
    let wurm = g.put(0, WURM, Zone::Battlefield);
    let rats = g.put(1, RATS, Zone::Battlefield);
    all_pass(&mut g);
    all_pass(&mut g);
    assert_eq!(g.decision, Some(Decision::Attackers { player: 0 }));
    g.act(0, Move::Attack(vec![(wurm, Target::Player(1))])).unwrap();
    all_pass(&mut g);
    assert_eq!(g.decision, Some(Decision::Blockers { player: 1 }));
    g.act(1, Move::Block(vec![(rats, wurm)])).unwrap();
    all_pass(&mut g);
    assert_eq!(g.part, Part::CombatDamage);
    // 1 damage to a creature with toughness 4
    assert!(g.log.contains(&Event::DamageDealt { source: rats, to: Target::Object(wurm), amount: 1, combat: true }));
    assert!(!g.battlefield.contains(&wurm));
    cite!(["704.5h", "702.2b"]; assert!(g.log.contains(&Event::Destroyed { object: wurm })));
    assert!(!g.battlefield.contains(&rats));
}

#[test]
fn r104_5_a_player_who_left_the_game_cant_be_chosen_as_a_target() {
    // a player who has lost has left the game (104.3a, 800.4a): they are no player for "each
    // player", nor for "any target"
    let mut g = game_of(3);
    g.players[2].life = 3;
    mana(&mut g, 0, MOUNTAIN, 2);
    let bolt = g.put(0, BOLT, Zone::Hand);
    let shock = g.put(0, SHOCK, Zone::Hand);
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Player(2)]] })).unwrap();
    all_pass(&mut g);
    assert!(g.players[2].lost);
    assert!(g.winner().is_none(), "two players are still in the game");
    cite!(["104.5", "102.1"]; refused("an illegal target, that player: that player has left the game (115.1)"): g.act(0, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Player(2)]] })));
}

#[test]
fn r800_4a_when_a_player_leaves_the_game_the_objects_they_own_leave_it() {
    // "When a player leaves the game, all objects … owned by that player leave the game"
    let mut g = game_of(3);
    let bears = g.put(2, BEARS, Zone::Battlefield);
    g.players[2].life = 3;
    mana(&mut g, 0, MOUNTAIN, 1);
    let bolt = g.put(0, BOLT, Zone::Hand);
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Player(2)]] })).unwrap();
    all_pass(&mut g);
    assert!(g.players[2].lost);
    cite!(["800.4a"]; assert!(!g.battlefield.contains(&bears)));
}
