//! Rooms (709.5): a permanent split card with one shared type line. A half is locked until
//! the permanent has its unlocked designation, and a locked half has no name, mana cost or
//! rules text (709.5, 709.5c). The half cast as the spell enters unlocked (709.5d); its
//! controller may pay a locked half's mana cost to unlock it as a special action during their
//! main phase with the stack empty (116.2m, 709.5e); "When you unlock this door" triggers on
//! either (709.5h). Roaring Furnace // Steaming Sauna.
use crate::support::{cite,paid};
use game::card::CastAs;
use game::state::Decision;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::Part;

const ROOM: usize = 0;
const BEAR: usize = 1;
const ISLAND: usize = 2;
const FURNACE: usize = 0;
const SAUNA: usize = 1;

fn room() -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let halves: Vec<_> = faces.iter().filter(|f| f.name == "Roaring Furnace // Steaming Sauna").collect();
    let d = CardDef::from_faces(&halves).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{}", e.0));
    d
}

/// Player 0 in their precombat main phase with three cards in hand, player 1 with a 2/5.
fn game() -> (Game, ObjId) {
    let defs = vec![room(), CardDef::new("Wall", "{0}", "Creature — Wall", "", Some((2, 5))).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap()];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    for _ in 0..3 { g.put(0, ISLAND, Zone::Hand); }
    let wall = g.put(1, BEAR, Zone::Battlefield);
    (g, wall)
}
fn mana(g: &mut Game) { g.players[0].pool.colors = [9; 5]; g.players[0].pool.colorless = 9; }
/// Resolve the stack, aiming any trigger at `at`.
fn resolve(g: &mut Game, at: Option<ObjId>) {
    for _ in 0..30 {
        match g.decision.clone() {
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
            Some(Decision::Targets { .. }) => g.act(0, Move::ChooseTargets(vec![vec![Target::Object(at.expect("a target"))]])).unwrap(),
            Some(d) => panic!("{d:?}"),
        }
    }
}
/// Cast this half from player 0's hand and resolve it: the Room permanent.
fn cast(g: &mut Game, half: usize, at: Option<ObjId>,units:&str) -> ObjId {
    let card = g.put(0, ROOM, Zone::Hand);
    mana(g);
    g.act(0, paid(units,Move::CastAs(CastAs::Half(half), Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    resolve(g, at);
    *g.battlefield.iter().find(|o| g.obj(**o).card == Some(ROOM)).unwrap()
}

fn furnace_hits(g: &Game, room: ObjId) -> usize {
    g.log.iter().filter(|e| matches!(e, Event::Unlocked { object, half: FURNACE, .. } if *object == room)).count()
}

#[test]
fn r709_5d_the_half_cast_enters_unlocked_and_its_door_triggers() {
    let (mut g, wall) = game();
    let room = cast(&mut g,FURNACE,Some(wall),"CR");
    cite!(["709.5d"]; assert_eq!(g.obj(room).unlocked, [true, false]));
    assert!(g.log.contains(&Event::Unlocked { object: room, half: FURNACE, player: 0 }));
    // "this Room deals damage equal to the number of cards in your hand to target creature
    // an opponent controls": three
    cite!(["709.5h"]; assert_eq!(g.obj(wall).damage, 3));
    // only the unlocked half's name and mana cost (709.5)
    cite!(["709.5"]; assert_eq!(g.obj(room).chars.name, "Roaring Furnace"));
    cite!(["709.5"]; assert_eq!(g.obj(room).chars.mana_value, 2));
    assert_eq!(g.def(room).unwrap().abilities().len(), 1);
}

#[test]
fn r709_5e_a_locked_door_is_unlocked_for_its_mana_cost_and_only_that_door_triggers() {

    let (mut g, wall) = game();
    let room = cast(&mut g,FURNACE,Some(wall),"CR");
    cite!(["709.5e"]; refused("that door is already unlocked (709.5e)"): unchanged(&mut g,paid("CR",Move::Unlock { room, half: FURNACE })));
    mana(&mut g);
    let pool = g.players[0].pool.clone();
    g.act(0, paid("CCCUU",Move::Unlock { room, half: SAUNA })).unwrap();
    // a special action: it does not use the stack (116.2m)
    cite!(["116.1"]; assert!(g.stack.is_empty()));
    cite!(["709.5e"]; assert_eq!(g.obj(room).unlocked, [true, true]));
    cite!(["709.5e"]; assert_eq!(g.players[0].pool.colors[1], pool.colors[1] - 2, "{{3}}{{U}}{{U}}"));
    assert_eq!(g.obj(room).chars.name, "Roaring Furnace // Steaming Sauna");
    assert_eq!(g.obj(room).chars.mana_value, 7);
    // the prompt shows each door's cost as printed, not the two run together
    let shown = game::simulation::Simulation::new(g.clone()).prompt(0).unwrap().entities[&room].mana_cost.clone();
    assert_eq!(shown.as_deref(), Some("{1}{R} // {3}{U}{U}"));
    // Roaring Furnace's door was not unlocked again: no second trigger
    resolve(&mut g, Some(wall));
    cite!(["709.5h"]; assert_eq!(furnace_hits(&g, room), 1));
    assert_eq!(g.obj(wall).damage, 3);
    // nothing left to unlock
    for half in [FURNACE, SAUNA] { cite!(["709.5e"]; refused("that door is already unlocked (709.5e)"): unchanged(&mut g,paid(if half==FURNACE {"CR"} else {"CCCUU"},Move::Unlock { room, half }))); }
}

#[test]
fn r709_5h_a_door_unlocked_later_triggers_then() {
    let (mut g, wall) = game();
    let room = cast(&mut g,SAUNA,None,"CCCUU");
    cite!(["709.5d"]; assert_eq!(g.obj(room).unlocked, [false, true]));
    assert_eq!(g.obj(room).chars.name, "Steaming Sauna");
    // Roaring Furnace is locked: it has no text, and nothing hit the Wall
    cite!(["709.5"]; assert_eq!(g.obj(wall).damage, 0));
    mana(&mut g);
    g.act(0, paid("CR",Move::Unlock { room, half: FURNACE })).unwrap();
    cite!(["709.5h"]; assert_eq!(g.decision.as_ref().map(|d| matches!(d, Decision::Targets { .. })), Some(true)));
    resolve(&mut g, Some(wall));
    assert_eq!(g.obj(wall).damage, 3);
}

#[test]
fn r709_5_a_room_put_onto_the_battlefield_has_both_doors_locked_and_no_text() {
    let (mut g, wall) = game();
    let room = g.put(0, ROOM, Zone::Battlefield);
    cite!(["709.5d"]; assert_eq!(g.obj(room).unlocked, [false, false]));
    cite!(["709.5"]; assert_eq!(g.obj(room).chars.name, ""));
    assert_eq!(g.obj(room).chars.mana_value, 0);
    cite!(["709.5"]; assert!(g.def(room).unwrap().abilities().is_empty()));
    assert!(g.seen(room, None).colors.is_empty());
    cite!(["709.5a"]; assert!(g.obj(room).chars.types.contains(&ir::CardType::Enchantment)));
    mana(&mut g);
    for half in [FURNACE, SAUNA] { cite!(["709.5e"]; ok: g.clone().act(0, paid(if half==FURNACE{"CR"}else{"CCCUU"},Move::Unlock { room, half }))); }
    g.act(0, paid("CR",Move::Unlock { room, half: FURNACE })).unwrap();
    resolve(&mut g, Some(wall));
    assert_eq!(g.obj(wall).damage, 3);
    assert_eq!(g.obj(room).chars.name, "Roaring Furnace");
}

#[test]
fn r116_2m_unlocking_only_by_its_controller_in_their_main_phase_with_the_stack_empty() {

    let (mut g, _) = game();
    let room = g.put(0, ROOM, Zone::Battlefield);
    let theirs = g.put(1, ROOM, Zone::Battlefield);
    mana(&mut g);
    // not another player's Room
    cite!(["116.2m"]; refused("only its controller unlocks a door (116.2m)"): unchanged(&mut g,paid("CR",Move::Unlock { room: theirs, half: FURNACE })));
    // not with a spell on the stack
    let card = g.put(0, ROOM, Zone::Hand);
    g.act(0, paid("CR",Move::CastAs(CastAs::Half(FURNACE), Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    cite!(["116.2m"]; refused("unlock a door only during a main phase of your turn with the stack empty (116.2m)"): unchanged(&mut g,paid("CCCUU",Move::Unlock { room, half: SAUNA })));
    // not outside a main phase
    let mut g = game().0;
    let room = g.put(0, ROOM, Zone::Battlefield);
    while g.part == Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    mana(&mut g);
    cite!(["116.2m"]; refused("unlock a door only during a main phase of your turn with the stack empty (116.2m)"): unchanged(&mut g,paid("CCCUU",Move::Unlock { room, half: SAUNA })));
}

#[test]
fn r709_5c_steaming_sauna_works_only_while_unlocked() {
    for unlocked in [false, true] {
        let (mut g, _) = game();
        let room = g.put(0, ROOM, Zone::Battlefield);
        if unlocked {
            mana(&mut g);
            g.act(0, paid("CCCUU",Move::Unlock { room, half: SAUNA })).unwrap();
        }
        let hand = g.players[0].hand.len();
        while g.part != Part::End { g.act(g.priority, Move::Pass).unwrap(); }
        resolve(&mut g, None);
        // "At the beginning of your end step, draw a card." only while unlocked
        cite!(["709.5"]; case: format!("unlocked {unlocked}"); assert_eq!(g.players[0].hand.len(), hand + usize::from(unlocked), "unlocked: {unlocked}"));
    }
}

#[test]
fn r709_4_a_room_that_leaves_is_a_whole_card_again_and_returns_locked() {
    let (mut g, wall) = game();
    let room = cast(&mut g,FURNACE,Some(wall),"CR");
    g.move_object(room, Zone::Hand);
    let card = *g.players[0].hand.last().unwrap();
    cite!(["709.4"]; assert_eq!(g.obj(card).chars.name, "Roaring Furnace // Steaming Sauna"));
    cite!(["709.4b"]; assert_eq!(g.obj(card).chars.mana_value, 7));
    g.move_object(card, Zone::Battlefield);
    let back = *g.battlefield.last().unwrap();
    cite!(["709.5d"]; assert_eq!(g.obj(back).unlocked, [false, false]));
    assert_eq!(g.obj(back).chars.name, "");
}

#[track_caller]
fn unchanged(g: &mut Game, action: Move) -> Result<(), String> {
    let before=format!("{g:?}"); let result=g.act(0,action);
    assert_eq!(format!("{g:?}"),before); result
}
