//! One simultaneous life-gain event per lifelink source (119.9, 702.15e).
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
fn defs(source_text: &str) -> Vec<CardDef> {
    vec![
        CardDef::new("Pulse", "{0}", "Creature", source_text, Some((4, 4))).unwrap(),
        CardDef::new(
            "Ajani's Pridemate",
            "{1}{W}",
            "Creature — Cat Soldier",
            "Whenever you gain life, put a +1/+1 counter on this creature.",
            Some((2, 2)),
        )
        .unwrap(),
    ]
}
fn game(source_text: &str) -> Game {
    let defs = defs(source_text);
    for d in &defs {
        game::play::check(d).unwrap();
    }
    Game::new(defs, 3, 20)
}
fn resolve(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn gains(g: &Game) -> Vec<(usize, u32)> {
    g.log
        .iter()
        .filter_map(|e| match e {
            Event::LifeGained { player, amount } => Some((*player, *amount)),
            _ => None,
        })
        .collect()
}
#[test]
fn r119_9_one_lifelink_source_damaging_multiple_players_causes_one_life_gain_event() {
    let mut g = game("Lifelink\n{0}: This creature deals 1 damage to each opponent.");
    let source = g.put(0, 0, Zone::Battlefield);
    let watcher = g.put(0, 1, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 1, targets: vec![], paying: vec![] }).unwrap();
    resolve(&mut g);
    cite!(["702.15b"]; assert_eq!(gains(&g), vec![(0, 2)]));
    assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![22, 19, 19]);
    cite!(["119.9"]; assert_eq!(g.stack.len(), 1));
    resolve(&mut g);
    assert_eq!(g.power(watcher), Some(3));
}
#[test]
fn r120_3_lifelink_is_determined_before_simultaneous_damage_changes_life_totals() {
    let mut g = game(
        "As long as you have 20 or more life, this creature has lifelink.\n{0}: This creature deals 1 damage to each player.",
    );
    let source = g.put(0, 0, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 1, targets: vec![], paying: vec![] }).unwrap();
    resolve(&mut g);
    cite!(["120.3f"]; assert_eq!(gains(&g), vec![(0, 3)]));
    assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![22, 19, 19]);
}

fn combat(g: &mut Game, attackers: &[ObjId], blocks: &[(ObjId, ObjId)]) {
    resolve(g);
    resolve(g);
    g.act(0, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap();
    resolve(g);
    if g.decision == Some(game::state::Decision::Blockers { player: 1 }) {
        g.act(1, Move::Block(blocks.to_vec())).unwrap();
    }
    resolve(g);
}

#[test]
fn r702_15e_two_simultaneous_lifelink_sources_trigger_pridemate_twice() {
    let mut g = game("Lifelink");
    let a = g.put(0, 0, Zone::Battlefield);
    let b = g.put(0, 0, Zone::Battlefield);
    let watcher = g.put(0, 1, Zone::Battlefield);
    combat(&mut g, &[a, b], &[]);
    cite!(["702.15e"]; assert_eq!(gains(&g), vec![(0, 4), (0, 4)]));
    cite!(["702.15e", "119.9"]; assert_eq!(g.stack.len(), 2));
    resolve(&mut g);
    resolve(&mut g);
    assert_eq!(g.power(watcher), Some(4));
}

#[test]
fn r119_9_trample_to_a_creature_and_player_triggers_pridemate_once() {
    let mut g = game("Lifelink, trample");
    let source = g.put(0, 0, Zone::Battlefield);
    let watcher = g.put(0, 1, Zone::Battlefield);
    let blocker = g.put(1, 1, Zone::Battlefield);
    combat(&mut g, &[source], &[(blocker, source)]);
    g.act(0, Move::Divide(vec![(Target::Object(blocker), 2), (Target::Player(1), 2)])).unwrap();
    cite!(["702.15b"]; assert_eq!(gains(&g), vec![(0, 4)]));
    assert_eq!(g.players[1].life, 18);
    cite!(["119.9"]; assert_eq!(g.stack.len(), 1));
    resolve(&mut g);
    assert_eq!(g.power(watcher), Some(3));
}

#[test]
fn r702_4b_double_strike_has_a_lifelink_event_in_each_damage_step() {
    let mut g = game("Lifelink, double strike");
    let source = g.put(0, 0, Zone::Battlefield);
    combat(&mut g, &[source], &[]);
    cite!(["702.4b"]; assert_eq!(gains(&g), vec![(0, 4)]));
    resolve(&mut g);
    cite!(["702.4b"]; assert_eq!(gains(&g), vec![(0, 4), (0, 4)]));
    assert_eq!(g.players[1].life, 12);
}

#[test]
fn r608_2c_separate_damage_instructions_cause_separate_lifelink_events() {
    let mut g = game(
        "Lifelink\n{0}: This creature deals 1 damage to each opponent. This creature deals 1 damage to each opponent.",
    );
    let source = g.put(0, 0, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 1, targets: vec![], paying: vec![] }).unwrap();
    resolve(&mut g);
    cite!(["608.2c"]; assert_eq!(gains(&g), vec![(0, 2), (0, 2)]));
}

#[test]
fn r702_15c_departed_source_uses_lifelink_from_last_known_information() {
    let mut g = game(
        "As long as you have 20 or more life, this creature has lifelink.\nSacrifice this creature: This creature deals 1 damage to each opponent.",
    );
    let source = g.put(0, 0, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 1, targets: vec![], paying: vec![] }).unwrap();
    g.lose_life(0, 1).unwrap();
    resolve(&mut g);
    cite!(["702.15c"]; assert_eq!(gains(&g), vec![(0, 2)]));
    assert_eq!(g.players[0].life, 21);
}

#[test]
fn r702_16e_prevented_damage_is_excluded_from_the_lifelink_total() {
    let mut d = defs("Lifelink\n{0}: This creature deals 1 damage to each creature.");
    d[0] = CardDef::new(
        "Pulse",
        "{R}",
        "Creature",
        "Lifelink\n{0}: This creature deals 1 damage to each creature.",
        Some((4, 4)),
    )
    .unwrap();
    d.push(CardDef::new("Guard", "{W}", "Creature", "Protection from red", Some((1, 1))).unwrap());
    for card in &d {
        game::play::check(card).unwrap();
    }
    let mut g = Game::new(d, 3, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.put(0, 1, Zone::Battlefield);
    let protected = g.put(1, 2, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 1, targets: vec![], paying: vec![] }).unwrap();
    resolve(&mut g);
    cite!(["702.16e", "702.15b"]; assert_eq!(gains(&g), vec![(0, 2)]));
    cite!(["702.16e"]; assert_eq!(g.obj(protected).damage, 0));
    assert!(!g.log.iter().any(|e| matches!(e, Event::DamageDealt { to: Target::Object(o), .. } if *o == protected)));
}

#[test]
fn r119_7_lifelink_respects_gain_prohibitions() {
    let mut g = game(
        "Lifelink\nPlayers can't gain life.\n{0}: This creature deals 1 damage to each player.",
    );
    let source = g.put(0, 0, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 2, targets: vec![], paying: vec![] }).unwrap();
    resolve(&mut g);
    assert!(gains(&g).is_empty());
    assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![19, 19, 19]);
}

#[test]
fn r120_8_zero_damage_does_not_produce_a_lifelink_event() {
    let mut g = game("Lifelink");
    let source = g.put(0, 0, Zone::Battlefield);
    g.damage(source, Target::Player(1), 0);
    cite!(["120.8"]; assert!(g.log.is_empty()));
    assert!(gains(&g).is_empty());
}
