//! Speed (702.179, 702.178): any life an opponent loses during your turn counts,
//! whoever caused it, combat damage too; two players with speed each count on their own turns;
//! "your speed" in a max speed ability is its controller's; max speed static, triggered and
//! activated abilities of other cards.
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};
use ir::Part;

const BASHTRONAUT: usize = 0;
const DRAIN: usize = 1;
const STEAL: usize = 2;
const RAIDER: usize = 3;
const PRIDE: usize = 4;
const KICKOFF: usize = 5;
const BEAR: usize = 6;
const ISLAND: usize = 7;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_faces(&CardDef::catalog(&faces)[name]).unwrap();
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    let defs = vec![real("Burnout Bashtronaut"),
        c("Drain", "Instant", "Target player loses 1 life.", None),
        c("Steal", "Instant", "Gain control of target permanent.", None),
        real("Gastal Raider"), real("Pride of the Road"), real("Kickoff Celebrations"),
        c("Bear", "Creature — Bear", "", Some((2, 2))),
        real("Island")];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..20 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    for p in 0..2 { g.players[p].pool.colors = [20; 5]; g.players[p].pool.colorless = 20; }
    g
}
fn step(g: &mut Game) {
    match g.decision.clone() {
        Some(game::state::Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
        Some(game::state::Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
        Some(game::state::Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
        _ => g.act(g.priority, Move::Pass).unwrap(),
    }
}
fn settle(g: &mut Game) { while !g.stack.is_empty() || g.decision.is_some() { step(g); } }
/// `player`, with priority, makes `whom` lose 1 life.
fn drain(g: &mut Game, player: usize, whom: usize) {
    while g.priority != player { step(g); }
    let spell = g.put(player, DRAIN, Zone::Hand);
    g.act(player, Move::Cast { card: spell, targets: vec![vec![Target::Player(whom)]] }).unwrap();
    settle(g);
}
fn next_turn(g: &mut Game) {
    let turn = g.turn;
    while g.turn == turn || g.part != Part::PrecombatMain || !g.stack.is_empty() { step(g); }
    for p in 0..2 { g.players[p].pool.colors = [20; 5]; g.players[p].pool.colorless = 20; }
}
fn speed(g: &Game, p: usize) -> Option<u8> { g.players[p].speed }

/// "Whenever one or more opponents lose life during your turn": the opponent's own spell
/// counts as well (702.179d names no source).
#[test]
fn an_opponent_losing_life_by_their_own_spell_counts() {
    let mut g = game();
    g.put(0, BASHTRONAUT, Zone::Battlefield);
    g.act(0, Move::Pass).unwrap();
    cite!(["702.179a", "704.5aa"]; assert_eq!(speed(&g, 0), Some(1)));
    drain(&mut g, 1, 1);
    cite!(["702.179d"]; assert_eq!(speed(&g, 0), Some(2)));
}

#[test]
fn combat_damage_to_the_opponent_counts() {
    let mut g = game();
    let bash = g.put(0, BASHTRONAUT, Zone::Battlefield);
    next_turn(&mut g);
    next_turn(&mut g);
    assert_eq!(speed(&g, 0), Some(1));
    while g.decision != Some(game::state::Decision::Attackers { player: 0 }) { step(&mut g); }
    g.act(0, Move::Attack(vec![(bash, Target::Player(1))])).unwrap();
    while g.players[1].life == 20 { step(&mut g); }
    settle(&mut g);
    cite!(["702.179d"]; assert_eq!(speed(&g, 0), Some(2)));
}

/// Both players have speed: each one's increases on their own turn, and one's increase does
/// not use up the other's.
#[test]
fn two_players_with_speed_each_count_on_their_own_turns() {
    let mut g = game();
    g.put(0, BASHTRONAUT, Zone::Battlefield);
    g.put(1, BASHTRONAUT, Zone::Battlefield);
    g.act(0, Move::Pass).unwrap();
    assert_eq!((speed(&g, 0), speed(&g, 1)), (Some(1), Some(1)));
    drain(&mut g, 0, 1);
    drain(&mut g, 1, 0);
    cite!(["702.179d"]; assert_eq!((speed(&g, 0), speed(&g, 1)), (Some(2), Some(1)), "player 0's turn"));
    next_turn(&mut g);
    assert_eq!(g.active, 1);
    drain(&mut g, 0, 1);
    drain(&mut g, 1, 0);
    cite!(["702.179d"]; assert_eq!((speed(&g, 0), speed(&g, 1)), (Some(2), Some(2)), "player 1's turn"));
}

/// "Max speed — This creature has double strike" is "as long as your speed is 4": its
/// controller's. Stolen by a player with no speed, it loses double strike, and that player
/// gets speed 1 (704.5aa).
#[test]
fn max_speed_reads_its_controllers_speed() {
    let mut g = game();
    let bash = g.put(0, BASHTRONAUT, Zone::Battlefield);
    g.players[0].speed = Some(4);
    cite!(["702.178a"]; assert!(g.has(bash, "Double Strike")));
    let steal = g.put(1, STEAL, Zone::Hand);
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Cast { card: steal, targets: vec![vec![Target::Object(bash)]] }).unwrap();
    settle(&mut g);
    assert_eq!(g.controller(bash), 1);
    cite!(["702.178a"]; assert!(!g.has(bash, "Double Strike")));
    cite!(["702.179a", "704.5aa"]; assert_eq!(speed(&g, 1), Some(1)));
    assert_eq!(speed(&g, 0), Some(4), "speed stays with its player");
}

#[test]
fn gastal_raider_gets_plus_one_and_menace_at_max_speed() {
    let mut g = game();
    let raider = g.put(0, RAIDER, Zone::Battlefield);
    g.players[0].speed = Some(3);
    cite!(["702.178a"]; assert_eq!((g.power(raider), g.toughness(raider), g.has(raider, "Menace")), (Some(2), Some(1), false)));
    g.players[0].speed = Some(4);
    cite!(["702.178a"]; assert_eq!((g.power(raider), g.toughness(raider), g.has(raider, "Menace")), (Some(3), Some(2), true)));
}

/// "Max speed — At the beginning of combat on your turn, target creature or Vehicle you
/// control gains double strike until end of turn.": only at max speed.
#[test]
fn pride_of_the_road_triggers_only_at_max_speed() {
    for s in [3, 4] {
        let mut g = game();
        let pride = g.put(0, PRIDE, Zone::Battlefield);
        g.players[0].speed = Some(s);
        while g.part != Part::BeginningOfCombat { step(&mut g); }
        let triggered = g.decision.is_some() || !g.stack.is_empty();
        cite!(["702.178a"]; case: format!("speed {s}"); assert_eq!(triggered, s == 4, "speed {s}: {:?}", g.decision));
        let _ = pride;
    }
}

/// "Max speed — Sacrifice this enchantment: ...": an ability to activate only at max speed.
#[test]
fn kickoff_celebrations_can_be_sacrificed_only_at_max_speed() {
    let mut g = game();
    let kickoff = g.put(0, KICKOFF, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.players[0].speed = Some(4);
    let ability = 3; // printed Engines, entry, Max speed; derived sacrifice activation
    g.players[0].speed = Some(3);
    cite!(["702.178a"]; refused("that is not an activated ability"): unchanged(&mut g, |g| g.act(0, Move::Activate { source: kickoff, ability, targets: vec![], paying: vec![] })));
    g.players[0].speed = Some(4);
    g.act(0, Move::Activate { source: kickoff, ability, targets: vec![], paying: vec![] }).unwrap();
    while !g.stack.is_empty() {
        assert!(g.decision.is_none(), "the authored haste ability asks no choice");
        g.act(g.priority, Move::Pass).unwrap();
    }
    assert!(!g.battlefield.contains(&kickoff));
    assert!(g.has(bear, "Haste"));
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
