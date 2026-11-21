//! Rooms whose "When you unlock this door" is on the right half, so its
//! ability index differs between the half alone and the whole card (709.5h, 709.5j), and a
//! Room's other door working by index once both are unlocked. Funeral Room // Awakening Hall,
//! Bottomless Pool // Locker Room.
use crate::support::{cite,paid};
use game::card::CastAs;
use game::state::Decision;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::Part;

const FUNERAL: usize = 0;
const POOL: usize = 1;
const BEAR: usize = 2;
const ISLAND: usize = 3;
const KILL: usize = 4;
const LEFT: usize = 0;
const RIGHT: usize = 1;

fn room(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let halves: Vec<_> = faces.iter().filter(|f| f.name == name).collect();
    let d = CardDef::from_faces(&halves).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}

/// Player 0 in their precombat main phase, player 1 with a Bear.
fn game() -> (Game, ObjId) {
    let defs = vec![room("Funeral Room // Awakening Hall"), room("Bottomless Pool // Locker Room"),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
        CardDef::new("Kill", "{0}", "Instant", "Destroy target creature.", None).unwrap()];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let bear = g.put(1, BEAR, Zone::Battlefield);
    (g, bear)
}
fn mana(g: &mut Game) { g.players[0].pool.colors = [9; 5]; g.players[0].pool.colorless = 9; }
/// Resolve the stack, aiming any trigger at `at`, or at nothing.
fn resolve(g: &mut Game, at: Option<ObjId>) {
    for _ in 0..30 {
        match g.decision.clone() {
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
            Some(Decision::Targets { .. }) => g.act(0, Move::ChooseTargets(vec![at.map(Target::Object).into_iter().collect()])).unwrap(),
            Some(d) => panic!("{d:?}"),
        }
    }
}
fn cast(g: &mut Game, def: usize, half: usize, at: Option<ObjId>,units:&str) -> ObjId {
    let card = g.put(0, def, Zone::Hand);
    mana(g);
    g.act(0, paid(units,Move::CastAs(CastAs::Half(half), Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    resolve(g, at);
    *g.battlefield.iter().find(|o| g.obj(**o).card == Some(def)).unwrap()
}
fn bears_in_graveyard(g: &mut Game, n: usize) { for _ in 0..n { g.put(0, BEAR, Zone::Graveyard); } }
fn my_bears(g: &Game) -> usize { g.battlefield.iter().filter(|o| g.obj(**o).card == Some(BEAR) && g.controller(**o) == 0).count() }

/// Awakening Hall, the right door, unlocked while Funeral Room is: with both unlocked its
/// trigger is the second ability of the whole card, and still "this door".
#[test]
fn a_right_door_unlocked_after_the_left_triggers() {
    let (mut g, _) = game();
    let room = cast(&mut g,FUNERAL,LEFT,None,"CCB");
    bears_in_graveyard(&mut g, 2);
    mana(&mut g);
    g.act(0, paid("CCCCCCBB",Move::Unlock { room, half: RIGHT })).unwrap();
    resolve(&mut g, None);
    cite!(["709.5e"]; assert_eq!(g.obj(room).unlocked, [true, true]));
    cite!(["709.5h"]; assert_eq!(my_bears(&g), 2, "\"return all creature cards from your graveyard to the battlefield\""));
}

/// Awakening Hall cast as the spell triggers as it enters (709.5d, 709.5h); unlocking Funeral
/// Room later does not trigger it again.
#[test]
fn a_left_door_unlocked_after_the_right_does_not_trigger_the_right() {
    let (mut g, _) = game();
    bears_in_graveyard(&mut g, 1);
    let room = cast(&mut g,FUNERAL,RIGHT,None,"CCCCCCBB");
    cite!(["709.5h"]; assert_eq!(my_bears(&g), 1));
    bears_in_graveyard(&mut g, 1);
    mana(&mut g);
    g.act(0, paid("CCB",Move::Unlock { room, half: LEFT })).unwrap();
    resolve(&mut g, None);
    cite!(["709.5h"]; assert_eq!(my_bears(&g), 1, "Awakening Hall's door was not unlocked again"));
    assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Unlocked { object, .. } if *object == room)).count(), 2);
}

/// With both doors unlocked, Funeral Room's "Whenever a creature you control dies" works.
#[test]
fn the_left_door_works_with_both_unlocked() {
    let (mut g, _) = game();
    let room = cast(&mut g,FUNERAL,RIGHT,None,"CCCCCCBB");
    mana(&mut g);
    g.act(0, paid("CCB",Move::Unlock { room, half: LEFT })).unwrap();
    resolve(&mut g, None);
    let mine = g.put(0, BEAR, Zone::Battlefield);
    let life = (g.players[0].life, g.players[1].life);
    let kill = g.put(0, KILL, Zone::Hand);
    g.act(0, Move::Cast { card: kill, targets: vec![vec![Target::Object(mine)]] }).unwrap();
    resolve(&mut g, None);
    assert_eq!(g.obj(mine).zone, Zone::Gone);
    cite!(["709.5"]; assert_eq!((g.players[0].life, g.players[1].life), (life.0 + 1, life.1 - 1)));
}

/// Bottomless Pool's "return up to one target creature" with a Bear, then Locker Room
/// unlocked: the Pool does not trigger again.
#[test]
fn bottomless_pool_returns_once() {
    let (mut g, bear) = game();
    let room = cast(&mut g,POOL,LEFT,Some(bear),"U");
    assert_eq!(g.obj(bear).zone, Zone::Gone, "the Bear went to its owner's hand");
    cite!(["709.5h"]; assert_eq!(g.players[1].hand.len(), 1));
    let other = g.put(1, BEAR, Zone::Battlefield);
    mana(&mut g);
    g.act(0, paid("CCCCU",Move::Unlock { room, half: RIGHT })).unwrap();
    cite!(["709.5h"]; assert!(g.decision.is_none() && g.stack.is_empty(), "{:?}", g.decision));
    assert_eq!(g.obj(other).zone, Zone::Battlefield);
    assert_eq!(g.obj(room).chars.name, "Bottomless Pool // Locker Room");
}

/// A cloaked Room is a face-down 2/2 with no halves (708.2a, 701.58a): it has no door to
/// unlock (709.5e), and the opponent's view does not say it is a Room (708.5).
#[test]
fn a_face_down_room_has_no_doors() {
    let cloaker = CardDef::new("Cloaker", "{0}", "Sorcery", "Cloak the top card of your library.", None).unwrap();
    let defs = vec![room("Funeral Room // Awakening Hall"), cloaker,
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap()];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, 2, Zone::Library); } g.put(p, 0, Zone::Library); }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(&mut g, None);
    let down = *g.battlefield.iter().find(|o| g.face_down(**o)).expect("a cloaked card");
    assert_eq!(g.obj(down).card, Some(0), "the Room is what was cloaked");
    let theirs = game::snapshot::Snapshot::of(&g, Some(1)).objects.into_iter().find(|s| s.id == down).unwrap();
    cite!(["708.5"]; assert_eq!(theirs.permanent.unwrap().unlocked, None, "the opponent is not told it is a Room"));
    mana(&mut g);
    cite!(["709.5e"]; refused("only a Room has doors to unlock (709.5e)"): g.act(0, Move::Unlock { room: down, half: LEFT }));
    assert!(g.face_down(down));
}

/// A locked door has no mana cost: a Room with only Locker Room unlocked is blue for {4}{U},
/// mana value 5; with neither, colorless, mana value 0 (709.5, 202.2).
#[test]
fn a_room_is_the_color_and_mana_value_of_its_unlocked_doors() {
    let (mut g, _) = game();
    let room = cast(&mut g,POOL,RIGHT,None,"CCCCU");
    assert_eq!(g.obj(room).chars.name, "Locker Room");
    cite!(["709.5", "202.3"]; assert_eq!(g.obj(room).chars.mana_value, 5));
    cite!(["709.5", "202.2"]; assert_eq!(g.seen(room, None).colors, [ir::Color::Blue]));
    let locked = g.put(0, POOL, Zone::Battlefield);
    cite!(["709.5", "202.3"]; assert_eq!(g.obj(locked).chars.mana_value, 0));
    cite!(["709.5", "202.2"]; assert!(g.seen(locked, None).colors.is_empty()));
}
