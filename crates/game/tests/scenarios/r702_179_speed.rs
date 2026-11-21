//! Start your engines! and speed (702.179), max speed (702.178). A player who controls a
//! permanent with start your engines! and has no speed gets speed 1 (704.5aa). Their speed
//! then increases once on each of their turns when an opponent loses life (702.179d), to at
//! most 4. "Max speed — [Ability]" is "As long as your speed is 4, this object has
//! '[Ability]'" (702.178a). Burnout Bashtronaut, Avishkar Raceway.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::Part;

const BASHTRONAUT: usize = 0;
const RACEWAY: usize = 1;
const DRAIN: usize = 2;
const MEASURE: usize = 3;
const ISLAND: usize = 4;
const STUDENT: usize = 5;
const DOOM: usize = 6;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_faces(&CardDef::catalog(&faces)[name]).unwrap();
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    let defs = vec![real("Burnout Bashtronaut"), real("Avishkar Raceway"),
        c("Drain", "Instant", "Target player loses 1 life.", None),
        c("Measure", "Artifact", "{T}: You gain life equal to your speed.", None),
        real("Island"),
        c("Student", "Creature — Human", "Whenever you gain life during your turn, draw a card.", Some((1, 1))),
        c("Doom", "Instant", "Destroy target creature.", None)];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..20 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn settle(g: &mut Game) {
    while !g.stack.is_empty() || g.decision.is_some() {
        match g.decision.clone() {
            Some(game::state::Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}
/// `player` makes `whom` lose 1 life, and it resolves.
fn drain(g: &mut Game, player: usize, whom: usize) {
    let spell = g.put(player, DRAIN, Zone::Hand);
    g.act(player, Move::Cast { card: spell, targets: vec![vec![Target::Player(whom)]] }).unwrap();
    settle(g);
}
/// The next turn's precombat main phase.
fn next_turn(g: &mut Game) {
    let turn = g.turn;
    while g.turn == turn || g.part != Part::PrecombatMain || !g.stack.is_empty() {
        match g.decision {
            Some(game::state::Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(game::state::Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}
fn speed(g: &Game, p: usize) -> Option<u8> { g.players[p].speed }
/// A Bashtronaut of player 0's, who then has speed 1.
fn started() -> (Game, ObjId) {
    let mut g = game();
    let bash = g.put(0, BASHTRONAUT, Zone::Battlefield);
    g.act(0, Move::Pass).unwrap();
    // the state-based action is checked as a player would receive priority
    assert_eq!(speed(&g, 0), Some(1));
    // on to the beginning of combat, where its player has priority again
    g.act(1, Move::Pass).unwrap();
    (g, bash)
}

#[test]
fn r704_5aa_controlling_start_your_engines_gives_speed_1() {
    let mut g = game();
    cite!(["702.179b", "702.179f"]; assert_eq!((speed(&g, 0), g.speed(0)), (None, 0), "no speed: 0 to what reads it (702.179b, f)"));
    g.put(0, BASHTRONAUT, Zone::Battlefield);
    g.act(0, Move::Pass).unwrap();
    cite!(["702.179a", "704.5aa"]; assert_eq!((speed(&g, 0), speed(&g, 1)), (Some(1), None)));
    assert!(g.log.iter().any(|e| matches!(e, Event::SpeedChanged { player: 0, speed: 1, previous: None })));
}

#[test]
fn r702_179d_speed_increases_once_each_of_its_players_turns_an_opponent_loses_life() {
    let (mut g, _) = started();
    drain(&mut g, 0, 1);
    cite!(["702.179d"]; assert_eq!(speed(&g, 0), Some(2)));
    drain(&mut g, 0, 1);
    cite!(["702.179d"]; assert_eq!(speed(&g, 0), Some(2), "only once each turn"));
    // losing life oneself is not an opponent's
    next_turn(&mut g);
    next_turn(&mut g);
    drain(&mut g, 0, 0);
    cite!(["702.179d"]; assert_eq!(speed(&g, 0), Some(2)));
    drain(&mut g, 0, 1);
    cite!(["702.179d"]; assert_eq!(speed(&g, 0), Some(3)));
}

#[test]
fn r702_179d_not_on_an_opponents_turn() {
    let (mut g, _) = started();
    next_turn(&mut g);
    assert_eq!(g.active, 1);
    drain(&mut g, 1, 1);
    // player 0 drains player 1 during player 1's turn
    g.act(1, Move::Pass).unwrap();
    drain(&mut g, 0, 1);
    cite!(["702.179d"]; assert_eq!(speed(&g, 0), Some(1)));
}

#[test]
fn r702_179e_speed_stops_at_4_and_max_speed_grants_double_strike() {
    let (mut g, bash) = started();
    assert!(!g.has(bash, "Double Strike"));
    for expected in 2..=4 {
        drain(&mut g, 0, 1);
        assert_eq!(speed(&g, 0), Some(expected));
        if expected < 4 { assert!(!g.has(bash, "Double Strike")); }
        next_turn(&mut g);
        next_turn(&mut g);
    }
    cite!(["702.178a"]; assert!(g.has(bash, "Double Strike"), "702.178a"));
    let changes = g.log.iter().filter(|e| matches!(e, Event::SpeedChanged { .. })).count();
    drain(&mut g, 0, 1);
    cite!(["702.179d"]; assert_eq!(speed(&g, 0), Some(4)));
    cite!(["603.4"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::SpeedChanged { .. })).count(), changes, "the ability doesn't trigger at 4 (603.4)"));
}

#[test]
fn r702_179b_speed_stays_once_its_permanent_is_gone() {
    let (mut g, bash) = started();
    drain(&mut g, 0, 1);
    let doom = g.put(0, DOOM, Zone::Hand);
    g.act(0, Move::Cast { card: doom, targets: vec![vec![Target::Object(bash)]] }).unwrap();
    settle(&mut g);
    assert!(!g.battlefield.contains(&bash));
    next_turn(&mut g);
    next_turn(&mut g);
    assert_eq!(speed(&g, 0), Some(2));
    drain(&mut g, 0, 1);
    cite!(["702.179d"]; assert_eq!(speed(&g, 0), Some(3), "a player with speed has the rule's ability"));
}

#[test]
fn r702_178a_a_max_speed_activated_ability_exists_only_at_max_speed() {
    let mut g = game();
    let raceway = g.put(0, RACEWAY, Zone::Battlefield);
    g.players[0].speed = Some(4);
    let ability = 3; // printed Engines, mana, Max speed; derived discard/draw activation
    g.players[0].speed = Some(3);
    g.players[0].pool.colorless = 3;
    let discard = g.put(0, ISLAND, Zone::Hand);
    cite!(["702.178a"]; refused("that is not an activated ability"): unchanged(&mut g, |g| g.act(0, crate::support::paid("CCC",Move::Activate { source: raceway, ability, targets: vec![], paying: vec![vec![discard]] }))));
    g.players[0].speed = Some(4);
    cite!(["702.178a"]; ok: g.act(0, crate::support::paid("CCC",Move::Activate { source: raceway, ability, targets: vec![], paying: vec![vec![discard]] })));
    assert!(g.obj(raceway).tapped && !g.players[0].hand.contains(&discard));
    assert_eq!((g.players[0].pool.total(), g.stack.len()), (0, 1));
}

#[test]
fn r702_179f_where_x_is_your_speed() {
    let mut g = game();
    let measure = g.put(0, MEASURE, Zone::Battlefield);
    g.act(0, Move::Activate { source: measure, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    settle(&mut g);
    cite!(["702.179f"]; assert_eq!(g.players[0].life, 20, "no speed is 0"));
    g.players[0].speed = Some(3);
    g.untap(measure);
    g.act(0, Move::Activate { source: measure, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    settle(&mut g);
    cite!(["702.179f"]; assert_eq!(g.players[0].life, 23));
}

/// "during your turn" (603.2): whose turn it is as the event happens.
#[test]
fn r603_2_a_trigger_during_your_turn() {
    let mut g = game();
    g.put(0, STUDENT, Zone::Battlefield);
    let measure = g.put(0, MEASURE, Zone::Battlefield);
    g.players[0].speed = Some(1);
    let hand = g.players[0].hand.len();
    g.act(0, Move::Activate { source: measure, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    settle(&mut g);
    cite!(["603.2"]; assert_eq!(g.players[0].hand.len(), hand + 1));
    next_turn(&mut g);
    let hand = g.players[0].hand.len();
    g.act(1, Move::Pass).unwrap();
    g.untap(measure);
    g.act(0, Move::Activate { source: measure, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    settle(&mut g);
    cite!(["603.2"]; assert_eq!(g.players[0].hand.len(), hand, "not during an opponent's turn"));
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
