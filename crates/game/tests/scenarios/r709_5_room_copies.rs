//! Room halves are copiable (709.5); unlocked designations belong to the copy (707.2).
use crate::support::{cite, mana_pool, oracle, paid};
use game::state::Decision;
use game::{Game, Move, ObjId, Target, Zone};

const POOL: usize = 0;
const MEAT: usize = 1;
const BEAR: usize = 2;
const MIRROR: usize = 3;
const REPLICATION: usize = 4;
const ISLAND: usize = 5;
const TEMPORARY: usize = 6;
const COPY_ENCHANTMENT: usize = 7;

fn game() -> Game {
    let mut defs: Vec<_> = [
        "Bottomless Pool // Locker Room", "Meat Locker // Drowned Diner", "Grizzly Bears",
        "Mirrorform", "Extravagant Replication", "Island",
    ].into_iter().map(oracle).collect();
    let temporary = game::CardDef::new("Temporary copy", "{0}", "Sorcery",
        "Target nonland permanent becomes a copy of target nonland permanent until end of turn.", None).unwrap();
    game::play::check(&temporary).unwrap();
    defs.push(temporary);
    defs.push(oracle("Copy Enchantment"));
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..8 { g.put(p, ISLAND, Zone::Library); } }
    g
}

fn resolve(g: &mut Game) {
    for _ in 0..20 {
        match &g.decision {
            // Both tested left doors have up to one target. Choose none explicitly.
            Some(Decision::Targets { player, .. }) => g.act(*player, Move::ChooseTargets(vec![vec![]])).unwrap(),
            Some(other) => panic!("unexpected decision: {other:?}"),
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("stack did not settle");
}

fn unlock(g: &mut Game, room: ObjId, half: usize, spending: &str) {
    g.players[0].pool = mana_pool(spending);
    g.act(0, paid(spending, Move::Unlock { room, half })).unwrap();
    resolve(g);
}

fn mirror(g: &mut Game, model: ObjId) {
    let card = g.put(0, MIRROR, Zone::Hand);
    g.players[0].pool = mana_pool("CCCCUU");
    g.act(0, paid("CCCCUU", Move::Cast { card, targets: vec![vec![Target::Object(model)]] })).unwrap();
    resolve(g);
}

fn assert_room(g: &Game, room: ObjId, unlocked: [bool; 2], name: &str, value: u32, cost: Option<&str>) {
    cite!(["709.5", "707.2"]; assert_eq!(g.obj(room).unlocked, unlocked));
    cite!(["709.5"]; assert_eq!(g.obj(room).chars.name, name));
    cite!(["709.5", "202.3"]; assert_eq!(g.obj(room).chars.mana_value, value));
    cite!(["709.5", "202.2"]; assert_eq!(g.seen(room, None).colors,
        if unlocked == [false, false] { vec![] } else { vec![ir::Color::Blue] }));
    let prompt = game::simulation::Simulation::new(g.clone()).prompt(0).unwrap();
    cite!(["709.5"]; assert_eq!(prompt.entities[&room].mana_cost.as_deref(), cost));
    let shown = game::snapshot::Snapshot::of(g, Some(1)).objects.into_iter().find(|o| o.id == room).unwrap();
    cite!(["709.5c"]; assert_eq!(shown.permanent.unwrap().unlocked, Some(unlocked)));
}

#[test]
fn r709_5e_a_room_copying_a_creature_has_no_doors_and_refuses_without_payment() {
    let mut g = game();
    let room = g.put(0, POOL, Zone::Battlefield);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    mirror(&mut g, bear);
    g.players[0].pool = mana_pool("U");
    let before = format!("{g:?}");
    cite!(["709.5e"]; refused("only a Room has doors to unlock (709.5e)"):
        g.act(0, paid("U", Move::Unlock { room, half: 0 })));
    cite!(["709.5e"]; assert_eq!(format!("{g:?}"), before));
    assert_eq!(g.obj(room).chars.name, "Grizzly Bears");
}

#[test]
fn r709_5e_a_room_copy_uses_the_copied_doors_cost_text_and_combined_values() {
    let mut g = game();
    let room = g.put(0, MEAT, Zone::Battlefield);
    let model = g.put(1, POOL, Zone::Battlefield);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    mirror(&mut g, model);
    g.players[0].pool = mana_pool("U");
    g.act(0, paid("U", Move::Unlock { room, half: 0 })).unwrap();
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bear)]])).unwrap();
    resolve(&mut g);
    cite!(["709.5h", "400.7"]; assert_eq!(g.obj(bear).zone, Zone::Gone));
    assert_eq!(g.players[1].hand.len(), 1, "the copied Pool returns the creature");
    assert_room(&g, room, [true, false], "Bottomless Pool", 1, Some("{U}"));
    unlock(&mut g, room, 1, "CCCCU");
    assert_room(&g, room, [true, true], "Bottomless Pool // Locker Room", 6, Some("{U} // {4}{U}"));
    cite!(["709.5h"]; assert_eq!(g.def(room).unwrap().abilities().len(), 2));
}

#[test]
fn r709_5_a_creature_that_copies_a_room_can_unlock_its_doors() {
    let mut g = game();
    let room = g.put(0, BEAR, Zone::Battlefield);
    let model = g.put(1, POOL, Zone::Battlefield);
    mirror(&mut g, model);
    assert_room(&g, room, [false, false], "", 0, None);
    unlock(&mut g, room, 0, "U");
    assert_room(&g, room, [true, false], "Bottomless Pool", 1, Some("{U}"));
}

fn replicate(g: &mut Game, model: ObjId) -> ObjId {
    g.put(0, REPLICATION, Zone::Battlefield);
    for _ in 0..100 {
        if matches!(g.decision, Some(Decision::Targets { player: 0, .. })) {
            g.act(0, Move::ChooseTargets(vec![vec![Target::Object(model)]])).unwrap();
            resolve(g);
            return *g.battlefield.iter().find(|o| g.obj(**o).token()).expect("Replication created a token");
        }
        match g.decision {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(ref other) => panic!("unexpected decision: {other:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("no Replication upkeep trigger");
}

#[test]
fn r709_5e_a_token_copy_of_a_room_has_its_own_unlockable_doors() {
    let mut g = game();
    let model = g.put(0, POOL, Zone::Battlefield);
    let room = replicate(&mut g, model);
    assert_room(&g, room, [false, false], "", 0, None);
    // Finish upkeep and draw to use the unlock special action in the main phase.
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    unlock(&mut g, room, 0, "U");
    assert_room(&g, room, [true, false], "Bottomless Pool", 1, Some("{U}"));
}

#[test]
fn r709_5d_a_room_token_copies_both_halves_but_no_unlocked_designations() {
    let mut g = game();
    let model = g.put(0, POOL, Zone::Battlefield);
    unlock(&mut g, model, 0, "U");
    let room = replicate(&mut g, model);
    cite!(["709.5d", "707.2"]; assert_eq!(g.obj(room).unlocked, [false, false]));
    assert_room(&g, room, [false, false], "", 0, None);
    assert_room(&g, model, [true, false], "Bottomless Pool", 1, Some("{U}"));
}

#[test]
fn r709_5c_a_room_copy_keeps_its_existing_unlocked_designation() {
    let mut g = game();
    let room = g.put(0, MEAT, Zone::Battlefield);
    unlock(&mut g, room, 0, "CCU");
    let model = g.put(1, POOL, Zone::Battlefield);
    mirror(&mut g, model);
    assert_room(&g, room, [true, false], "Bottomless Pool", 1, Some("{U}"));
    cite!(["707.4", "709.5h"]; assert!(g.stack.is_empty() && g.decision.is_none(), "copying does not unlock a door again"));
}

#[test]
fn r709_5c_expiring_a_copy_reads_the_rooms_current_designations() {
    let mut g = game();
    let room = g.put(0, POOL, Zone::Battlefield);
    let model = g.put(1, MEAT, Zone::Battlefield);
    let card = g.put(0, TEMPORARY, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(room)], vec![Target::Object(model)]] }).unwrap();
    resolve(&mut g);
    unlock(&mut g, room, 0, "CCU");
    cite!(["709.5"]; assert_eq!(g.obj(room).chars.name, "Meat Locker"));
    let mut branch = g.clone();
    for g in [&mut g, &mut branch] {
        let turn = g.turn;
        for _ in 0..40 {
            if g.turn != turn { break; }
            match g.decision {
                Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
                Some(ref other) => panic!("unexpected decision: {other:?}"),
                None => g.act(g.priority, Move::Pass).unwrap(),
            }
        }
        assert_ne!(g.turn, turn);
        cite!(["514.2", "709.5c"]; assert_eq!(g.obj(room).unlocked, [true, false]));
        assert_room(g, room, [true, false], "Bottomless Pool", 1, Some("{U}"));
    }
}

#[test]
fn r709_5d_a_card_entering_as_a_room_copy_starts_with_neither_door_unlocked() {
    let mut g = game();
    let model = g.put(0, POOL, Zone::Battlefield);
    unlock(&mut g, model, 0, "U");
    let card = g.put(0, COPY_ENCHANTMENT, Zone::Hand);
    g.players[0].pool = mana_pool("CCU");
    g.act(0, paid("CCU", Move::Cast { card, targets: vec![] })).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Choose(game::state::Answer::Objects(vec![model]))).unwrap();
    resolve(&mut g);
    let room = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(COPY_ENCHANTMENT)).unwrap();
    cite!(["709.5d", "707.2"]; assert_eq!(g.obj(room).unlocked, [false, false]));
    assert_room(&g, room, [false, false], "", 0, None);
    unlock(&mut g, room, 0, "U");
    assert_room(&g, room, [true, false], "Bottomless Pool", 1, Some("{U}"));
}

#[test]
fn r707_9_room_copy_exceptions_keep_both_doors_and_their_own_triggers() {
    let mut g = game();
    let copier = game::CardDef::new("Room copy", "{0}", "Sorcery",
        "Create a token that's a copy of target enchantment, except it has hexproof.", None).unwrap();
    game::play::check(&copier).unwrap();
    let index = g.defs.len();
    std::sync::Arc::make_mut(&mut g.defs).push(copier);
    let model = g.put(0, POOL, Zone::Battlefield);
    unlock(&mut g, model, 0, "U");
    let spell = g.put(0, index, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(model)]] }).unwrap();
    resolve(&mut g);
    let room = *g.battlefield.iter().find(|o| g.obj(**o).token()).unwrap();
    assert_room(&g, room, [false, false], "", 0, None);
    cite!(["707.9a", "709.5"]; assert!(g.has(room, "Hexproof")));
    let bear = g.put(1, BEAR, Zone::Battlefield);
    g.players[0].pool = mana_pool("U");
    g.act(0, paid("U", Move::Unlock { room, half: 0 })).unwrap();
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bear)]])).unwrap();
    resolve(&mut g);
    assert_room(&g, room, [true, false], "Bottomless Pool", 1, Some("{U}"));
    cite!(["709.5h", "400.7"]; assert_eq!(g.obj(bear).zone, Zone::Gone));
    cite!(["707.9a"]; assert!(g.has(room, "Hexproof")));
    unlock(&mut g, room, 1, "CCCCU");
    assert_room(&g, room, [true, true], "Bottomless Pool // Locker Room", 6, Some("{U} // {4}{U}"));
    cite!(["707.9a"]; assert!(g.has(room, "Hexproof")));
    // A second copy retains the modified halves, without inheriting either designation.
    let spell = g.put(0, COPY_ENCHANTMENT, Zone::Hand);
    g.players[0].pool = mana_pool("CCU");
    g.act(0, paid("CCU", Move::Cast { card: spell, targets: vec![] })).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Choose(game::state::Answer::Objects(vec![room]))).unwrap();
    resolve(&mut g);
    let second = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(COPY_ENCHANTMENT)).unwrap();
    assert_room(&g, second, [false, false], "", 0, None);
    unlock(&mut g, second, 1, "CCCCU");
    assert_room(&g, second, [false, true], "Locker Room", 5, Some("{4}{U}"));
    cite!(["707.2", "707.9a"]; assert!(g.has(second, "Hexproof")));
}
