//! A delayed triggered ability made only if a condition holds (603.7a, 608.2c): "If this spell
//! was kicked, sacrifice that creature at the beginning of the next end step." And what such an
//! ability does to the permanent it returns: "If you do, return it to the battlefield … at the
//! beginning of your next upkeep. It gains haste." (400.7)
use crate::support::{cite, mana_pool, oracle};
use game::state::{Answer, Decision};
use game::{Game, Move, ObjId, Target, Zone};
use ir::Part;

fn step(g: &mut Game) {
    match g.decision.clone() {
        Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
        Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
        _ => g.act(g.priority, Move::Pass).unwrap(),
    };
}

/// In Thrall to the Pit takes the opponent's Grizzly Bears, kicked or not; then the turn goes
/// on to the next turn's upkeep.
fn thrall(kicks: u32) -> (Game, game::ObjId) {
    let mut g = Game::new(vec![oracle("In Thrall to the Pit"), oracle("Grizzly Bears")], 2, 20);
    g.put(1, 1, Zone::Library);
    g.put(0, 1, Zone::Library);
    let bears = g.put(1, 1, Zone::Battlefield);
    let spell = g.put(0, 0, Zone::Hand);
    while g.part != Part::PrecombatMain { step(&mut g); }
    let mana = if kicks == 0 { "CCCR" } else { "CCCCCRB" };
    g.players[0].pool = mana_pool(mana);
    let cast = Move::WithKicks(kicks, Box::new(Move::Cast { card: spell, targets: vec![vec![Target::Object(bears)]] }));
    g.act(0, Move::Paying(mana_pool(mana), Box::new(cast))).unwrap();
    while !g.stack.is_empty() { step(&mut g); }
    assert_eq!(g.controller(bears), 0);
    (g, bears)
}

#[test]
fn r603_7a_kicked_the_creature_is_sacrificed_at_the_beginning_of_the_next_end_step() {
    let (mut g, _) = thrall(1);
    while g.part != Part::End { step(&mut g); }
    cite!(["603.7a", "702.33d"]; assert_eq!(g.stack.len(), 1));
    while !g.stack.is_empty() { step(&mut g); }
    cite!(["603.7a", "701.21a"]; assert_eq!(g.players[1].graveyard.len(), 1));
    cite!(["603.7a"]; assert!(g.battlefield.is_empty()));
}

#[test]
fn r608_2c_unkicked_no_delayed_trigger_is_made_and_the_creature_returns() {
    let (mut g, bears) = thrall(0);
    while g.part != Part::End { step(&mut g); }
    cite!(["608.2c", "603.7a"]; assert!(g.stack.is_empty()));
    while g.turn == 1 { step(&mut g); }
    cite!(["514.2"]; assert_eq!(g.controller(bears), 1));
    cite!(["608.2c"]; assert!(g.players[1].graveyard.is_empty()));
}

/// Obzedat, Ghost Council's end step trigger: "you may exile Obzedat. If you do, return it to
/// the battlefield under its owner's control at the beginning of your next upkeep. It gains
/// haste." Answered as asked.
fn obzedat(exile: bool) -> (Game, ObjId) {
    let mut g = Game::new(vec![oracle("Obzedat, Ghost Council"), oracle("Grizzly Bears")], 2, 20);
    for _ in 0..3 { g.put(0, 1, Zone::Library); g.put(1, 1, Zone::Library); }
    let obzedat = g.put(0, 0, Zone::Battlefield);
    while !(g.part == Part::End && !g.stack.is_empty()) { step(&mut g); }
    while matches!(g.asked(), Some((_, game::legal::Asked::Priority))) { step(&mut g); }
    cite!(["603.5"]; assert!(matches!(g.asked(), Some((0, game::legal::Asked::May))), "{:?}", g.asked()));
    g.act(0, Move::Choose(if exile { Answer::Yes } else { Answer::No })).unwrap();
    (g, obzedat)
}

#[test]
fn r400_7_the_permanent_returned_at_the_next_upkeep_is_the_one_that_gains_haste() {
    let (mut g, obzedat) = obzedat(true);
    cite!(["406.3"]; assert!(g.battlefield.is_empty() && g.exile.len() == 1));
    while !(g.turn == 3 && g.part == Part::Upkeep && !g.stack.is_empty()) { step(&mut g); }
    cite!(["603.7a"]; assert!(g.battlefield.is_empty()));
    while !g.stack.is_empty() && g.decision.is_none() { step(&mut g); }
    // it enters as a new object, whose own enters trigger targets an opponent (400.7, 603.6a)
    cite!(["603.6a"]; assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. })), "{:?}", g.decision));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    while !g.stack.is_empty() { step(&mut g); }
    cite!(["603.6a"]; assert_eq!(g.players[1].life, 18));
    cite!(["603.7a", "400.7"]; assert!(g.exile.is_empty() && g.battlefield.len() == 1 && g.battlefield[0] != obzedat));
    let returned = g.battlefield[0];
    cite!(["603.7a", "400.7"]; assert!(g.has(returned, "Haste")));
    // it can attack this turn (302.6, 702.10b)
    while !matches!(g.decision, Some(Decision::Attackers { player: 0 })) { step(&mut g); }
    cite!(["702.10b"]; assert!(g.act(0, Move::Attack(vec![(returned, Target::Player(1))])).is_ok()));
    // no duration is stated: it keeps haste (611.2a), past this turn's cleanup; its own end
    // step trigger is declined
    while g.turn == 3 {
        if matches!(g.asked(), Some((0, game::legal::Asked::May))) { g.act(0, Move::Choose(Answer::No)).unwrap(); } else { step(&mut g); }
    }
    cite!(["611.2a"]; assert!(g.has(returned, "Haste")));
}

#[test]
fn r608_2c_kept_on_the_battlefield_it_does_not_gain_haste() {
    let (mut g, obzedat) = obzedat(false);
    cite!(["608.2c"]; assert_eq!(g.battlefield, vec![obzedat]));
    cite!(["608.2c", "603.7a"]; assert!(!g.has(obzedat, "Haste")));
    while g.turn < 3 || g.part != Part::Draw { step(&mut g); }
    cite!(["608.2c", "603.7a"]; assert!(g.stack.is_empty() && g.battlefield == vec![obzedat] && !g.has(obzedat, "Haste")));
}

/// "If this ability has been activated four or more times this turn, sacrifice this creature at
/// the beginning of the next end step.": its first ability with this text activated this many
/// times in the first main phase, each resolved; then the turn goes on to its end step.
fn activated(name: &str, ability: usize, mana: &str, times: usize) -> (Game, ObjId) {
    let mut g = Game::new(vec![oracle(name)], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    while g.part != Part::PrecombatMain { step(&mut g); }
    for _ in 0..times {
        g.players[0].pool = mana_pool(mana);
        let activate = Move::Activate { source, ability, targets: vec![], paying: vec![] };
        g.act(0, Move::Paying(mana_pool(mana), Box::new(activate))).unwrap();
        while !g.stack.is_empty() { step(&mut g); }
    }
    while g.part != Part::End { step(&mut g); }
    (g, source)
}

#[test]
fn r603_7a_the_fourth_activation_in_a_turn_makes_the_delayed_sacrifice() {
    // Dragon Whelp: "{R}: This creature gets +1/+0 until end of turn. If this ability has been
    // activated four or more times this turn, sacrifice this creature …"
    let (g, whelp) = activated("Dragon Whelp", 1, "R", 3);
    cite!(["608.2c", "603.7a"]; assert!(g.stack.is_empty() && g.battlefield == vec![whelp]));
    let (mut g, _) = activated("Dragon Whelp", 1, "R", 4);
    cite!(["603.7a", "608.2c"]; assert_eq!(g.stack.len(), 1));
    while !g.stack.is_empty() { step(&mut g); }
    cite!(["603.7a", "701.21a"]; assert!(g.battlefield.is_empty() && g.players[0].graveyard.len() == 1));
    // each activation after the third makes its own (the rulings of Dragon Whelp)
    let (g, _) = activated("Dragon Whelp", 1, "R", 5);
    cite!(["603.7a"]; assert_eq!(g.stack.len(), 2));
}

#[test]
fn r605_3b_a_mana_ability_counts_as_activated_and_makes_its_delayed_sacrifice() {
    // Initiates of the Ebon Hand: "{1}: Add {B}. If this ability has been activated four or more
    // times this turn, sacrifice this creature at the beginning of the next end step."
    let (g, initiates) = activated("Initiates of the Ebon Hand", 0, "C", 3);
    cite!(["605.3b", "608.2c"]; assert!(g.stack.is_empty() && g.battlefield == vec![initiates]));
    let (mut g, _) = activated("Initiates of the Ebon Hand", 0, "C", 4);
    cite!(["605.3b", "603.7a"]; assert_eq!(g.stack.len(), 1));
    while !g.stack.is_empty() { step(&mut g); }
    cite!(["603.7a", "701.21a"]; assert!(g.battlefield.is_empty() && g.players[0].graveyard.len() == 1));
}
