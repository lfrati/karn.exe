//! What one sentence has the ability's own object both become and do (CR 608.2h, 113.7a,
//! 611.2c): "This creature gets +1/+0 until end of turn and deals 1 damage to you." The
//! continuous part is of the object as the ability resolves, so it is of nothing once the
//! object has left; what the object does it still does, as it last was.
use crate::support::cite;
use game::legal::Asked;
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const BEAR: usize = 1;

/// A 2/1 with this text under player 0's control, and a Bear of each player.
fn game(text: &str) -> (Game, ObjId) {
    let defs = vec![
        CardDef::new("Probe", "{R}", "Creature — Satyr", text, Some((2, 1))).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{text}: {}", u.0));
    }
    let mut g = Game::new(defs, 2, 20);
    for player in 0..2 { for _ in 0..5 { g.put(player, BEAR, Zone::Library); } }
    let probe = g.put(0, 0, Zone::Battlefield);
    (g, probe)
}
fn activate(g: &mut Game, source: ObjId, targets: Vec<Vec<Target>>) {
    let ability = g.def(source).unwrap().abilities().len() - 1;
    g.act(0, Move::Activate { source, ability, targets, paying: vec![] }).unwrap();
}
fn resolve(g: &mut Game) {
    while g.asked().is_some_and(|(_, a)| a == Asked::Priority) && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}
/// The ability resolved with its source there, and with its source gone to the graveyard
/// while the ability was on the stack.
fn both(text: &str) -> ((Game, ObjId), Game) {
    let (mut live, probe) = game(text);
    activate(&mut live, probe, vec![]);
    let mut departed = live.clone();
    departed.move_object(probe, Zone::Graveyard);
    resolve(&mut live);
    resolve(&mut departed);
    assert_eq!(departed.battlefield.len(), 0);
    ((live, probe), departed)
}
fn lives(g: &Game) -> (i32, i32) { (g.players[0].life, g.players[1].life) }

#[test]
fn the_damage_is_dealt_whichever_side_of_the_change_it_is_said_on() {
    for text in [
        "{0}: This creature gets +1/+0 until end of turn and deals 1 damage to you.",
        "{0}: This creature deals 1 damage to you and gets +1/+0 until end of turn.",
        "{0}: This creature gets +1/+0 until end of turn. This creature deals 1 damage to you.",
        "{0}: This creature gets +1/+0 until end of turn. It deals 1 damage to you.",
        "{0}: This creature gains trample until end of turn and deals 1 damage to you.",
        // two changes before the damage
        "{0}: This creature gets +1/+0 and gains trample until end of turn and deals 1 damage to you.",
    ] {
        let ((live, probe), departed) = both(text);
        cite!(["608.2h", "113.7a"]; case: text; assert_eq!((lives(&live), lives(&departed)), ((19, 20), (19, 20)), "{text}"));
        if text.contains("+1/+0") { assert_eq!(live.power(probe), Some(3), "{text}"); }
        if text.contains("trample") { assert!(live.has(probe, "Trample"), "{text}"); }
    }
}

#[test]
fn the_damage_goes_where_the_sentence_says() {
    let text = "{0}: This creature gets +1/+0 until end of turn and deals 1 damage to each opponent.";
    let ((live, _), departed) = both(text);
    cite!(["608.2h"]; assert_eq!((lives(&live), lives(&departed)), ((20, 19), (20, 19))));
    // a target: chosen as the ability is activated, hit after the source has left
    let text = "{0}: This creature gets +1/+0 until end of turn and deals 1 damage to target creature.";
    let (mut live, probe) = game(text);
    let bear = live.put(1, BEAR, Zone::Battlefield);
    activate(&mut live, probe, vec![vec![Target::Object(bear)]]);
    let mut departed = live.clone();
    departed.move_object(probe, Zone::Graveyard);
    resolve(&mut live);
    resolve(&mut departed);
    cite!(["608.2h"]; assert_eq!((live.obj(bear).damage, departed.obj(bear).damage, live.power(probe)), (1, 1, Some(3))));
}

#[test]
fn it_is_the_source_that_deals_it_as_it_last_was() {
    // its lifelink is that of the object that left (702.15b, 608.2h)
    let text = "Lifelink\n{0}: This creature gets +1/+0 until end of turn and deals 1 damage to each opponent.";
    let ((live, _), departed) = both(text);
    cite!(["702.15b", "608.2h"]; assert_eq!((lives(&live), lives(&departed)), ((21, 19), (21, 19))));
    // its power: with the change where it is there, without it where the change found nothing
    let text = "{0}: This creature gets +2/+0 until end of turn and deals damage equal to its power to each opponent.";
    let ((live, probe), departed) = both(text);
    cite!(["608.2h"]; assert_eq!((live.power(probe), live.players[1].life, departed.players[1].life), (Some(4), 16, 18)));
}

#[test]
fn the_object_it_became_elsewhere_is_not_changed() {
    // left and returned: a new object (400.7), which the change is not of; the old one deals
    let text = "{0}: This creature gets +1/+0 until end of turn and deals 1 damage to you.";
    let (mut g, probe) = game(text);
    activate(&mut g, probe, vec![]);
    g.move_object(probe, Zone::Exile);
    let exiled = g.exile[0];
    g.move_object(exiled, Zone::Battlefield);
    let [returned] = g.battlefield[..] else { panic!("it returned") };
    cite!(["400.7"]; assert_ne!(returned, probe));
    resolve(&mut g);
    cite!(["400.7", "608.2h"]; assert_eq!((g.power(returned), g.players[0].life, g.obj(returned).damage), (Some(2), 19, 0)));
    // an ability activated twice: each resolves by itself, the second after the source left
    let (mut g, probe) = game(text);
    activate(&mut g, probe, vec![]);
    activate(&mut g, probe, vec![]);
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    assert_eq!((g.power(probe), g.players[0].life, g.stack.len()), (Some(3), 19, 1));
    g.move_object(probe, Zone::Graveyard);
    resolve(&mut g);
    cite!(["113.7a"]; assert_eq!(g.players[0].life, 18));
}

#[test]
fn another_object_as_the_subject_does_nothing_once_it_is_gone() {
    // the target is the subject of both halves: an illegal target, and nothing is done (608.2b)
    let text = "{0}: Target creature gets +1/+0 until end of turn and deals 1 damage to you.";
    let (mut live, probe) = game(text);
    let bear = live.put(1, BEAR, Zone::Battlefield);
    activate(&mut live, probe, vec![vec![Target::Object(bear)]]);
    let (mut target_gone, mut source_gone) = (live.clone(), live.clone());
    target_gone.move_object(bear, Zone::Graveyard);
    source_gone.move_object(probe, Zone::Graveyard);
    for g in [&mut live, &mut target_gone, &mut source_gone] { resolve(g); }
    assert_eq!((live.power(bear), live.players[0].life), (Some(3), 19));
    cite!(["608.2b"]; assert_eq!(target_gone.players[0].life, 20));
    // the ability's own object leaving changes nothing: the Bear is there and deals it
    cite!(["113.7a"]; assert_eq!((source_gone.power(bear), source_gone.players[0].life), (Some(3), 19)));
}
