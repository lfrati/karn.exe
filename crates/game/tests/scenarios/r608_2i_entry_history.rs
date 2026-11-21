//! Historical entry conditions use event-time facts, not current board membership.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
fn setup() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let blaze = CardDef::from_face(faces.iter().find(|f| f.face_name == "Searing Blaze").unwrap()).unwrap();
    let defs = vec![
        blaze,
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((4, 4))).unwrap(),
        CardDef::planeswalker("Walker", "{0}", "Planeswalker — Jace", "+1: You gain 1 life.", 5).unwrap(),
        CardDef::new("Change", "{0}", "Instant", "Target land becomes a 2/2 creature until end of turn.", None)
            .unwrap(),
    ];
    let mut defs = defs;
    defs.push(CardDef::new("Control", "{0}", "Instant", "Gain control of target permanent.", None).unwrap());
    for d in &defs {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {e:?}", d.name()));
    }
    let mut g = Game::new(defs, 3, 20);
    for p in 0..3 {
        for _ in 0..10 {
            g.put(p, 1, Zone::Library);
        }
    }
    g
}
fn resolve(g: &mut Game) {
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn enter(g: &mut Game, player: usize) -> ObjId {
    let land = g.put(player, 1, Zone::Hand);
    g.move_object(land, Zone::Battlefield);
    *g.battlefield.last().unwrap()
}
fn cast(g: &mut Game, to: Target, creature: ObjId) {
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[3] = 2;
    g.act(0, paid("RR", Move::Cast { card, targets: vec![vec![to], vec![Target::Object(creature)]] })).unwrap();
}
#[test]
fn r608_2c_landfall_replaces_both_damage_amounts_rather_than_adding_them() {
    for (entry, expected) in [(None, 1), (Some(0), 3), (Some(1), 1)] {
        let mut g = setup();
        let creature = g.put(1, 2, Zone::Battlefield);
        if let Some(p) = entry {
            enter(&mut g, p);
        }
        cast(&mut g, Target::Player(1), creature);
        resolve(&mut g);
        cite!(["608.2c"]; case: format!("{entry:?}"); assert_eq!(g.players[1].life, 20 - expected));
        cite!(["608.2c"]; case: format!("{entry:?}"); assert_eq!(g.obj(creature).damage, expected));
    }
}
#[test]
fn r608_2i_entry_is_remembered_after_departure_and_after_its_type_changes() {
    for leave in [true, false] {
        let mut g = setup();
        let creature = g.put(1, 2, Zone::Battlefield);
        let land = enter(&mut g, 0);
        if leave {
            g.move_object(land, Zone::Graveyard);
        } else {
            let change = g.put(0, 4, Zone::Hand);
            g.act(0, Move::Cast { card: change, targets: vec![vec![Target::Object(land)]] }).unwrap();
            resolve(&mut g);
            assert!(!g.seen(land, None).is(ir::CardType::Land));
        }
        cast(&mut g, Target::Player(1), creature);
        resolve(&mut g);
        cite!(["608.2i"]; case: format!("leave={leave}"); assert_eq!(g.players[1].life, 17));
    }
}
#[test]
fn r608_2c_entry_after_casting_counts_at_resolution_and_survives_cloning() {
    let mut g = setup();
    let creature = g.put(1, 2, Zone::Battlefield);
    cast(&mut g, Target::Player(1), creature);
    let mut no_entry = g.clone();
    enter(&mut g, 0);
    resolve(&mut g);
    resolve(&mut no_entry);
    cite!(["608.2c", "608.2i"]; assert_eq!(g.players[1].life, 17));
    cite!(["608.2c", "608.2i"]; assert_eq!(no_entry.players[1].life, 19));
}
#[test]
fn r608_2b_losing_one_target_still_damages_the_other_and_relations_are_required() {
    let mut g = setup();
    enter(&mut g, 0);
    let creature = g.put(1, 2, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[3] = 2;
    cite!(["115.1"]; refused("an illegal target, Bear: the target is one the player it names controls, and another player controls it (115.1)"):
        g.act(0, Move::Cast { card, targets: vec![vec![Target::Player(2)], vec![Target::Object(creature)]] }));
    g.act(0, paid("RR", Move::Cast { card, targets: vec![vec![Target::Player(1)], vec![Target::Object(creature)]] })).unwrap();
    g.move_object(creature, Zone::Graveyard);
    resolve(&mut g);
    cite!(["608.2b"]; assert_eq!(g.players[1].life, 17));
}
#[test]
fn r115_1_the_first_target_can_be_a_planeswalker_whose_controller_controls_the_creature() {
    let mut g = setup();
    enter(&mut g, 0);
    let walker = g.put(1, 3, Zone::Battlefield);
    g.put_counters(walker, "loyalty", 5);
    let creature = g.put(1, 2, Zone::Battlefield);
    cast(&mut g, Target::Object(walker), creature);
    resolve(&mut g);
    cite!(["120.3c"]; assert_eq!(g.obj(walker).counters.get("loyalty"), Some(&2)));
    assert_eq!(g.obj(creature).damage, 3);
    assert_eq!(g.players[1].life, 20);
}
#[test]
fn r500_1_entry_summary_expires_at_the_next_turn() {
    let mut g = setup();
    enter(&mut g, 0);
    while g.active == 0 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    let creature = g.put(1, 2, Zone::Battlefield);
    g.act(g.priority, Move::Pass).unwrap();
    g.act(g.priority, Move::Pass).unwrap();
    assert_eq!(g.priority, 0);
    cast(&mut g, Target::Player(1), creature);
    resolve(&mut g);
    cite!(["608.2i"]; assert_eq!(g.players[1].life, 19));
}

#[test]
fn r608_2i_multiple_entries_and_token_entries_count_under_the_original_controller() {
    for token in [false, true] {
        let mut g = setup();
        let creature = g.put(1, 2, Zone::Battlefield);
        let land = if token {
            let model = g.put(0, 1, Zone::Hand);
            g.create_token(0, g.obj(model).chars.clone(), false)
        } else {
            enter(&mut g, 0)
        };
        // A controller change is not a new entry and cannot rewrite its event-time owner.
        let control = g.put(2, 5, Zone::Hand);
        while g.priority != 2 {
            g.act(g.priority, Move::Pass).unwrap();
        }
        g.act(2, Move::Cast { card: control, targets: vec![vec![Target::Object(land)]] }).unwrap();
        resolve(&mut g);
        assert_eq!(g.seen(land, None).controller, 2);
        if !token {
            let mut single = g.clone();
            cast(&mut single, Target::Player(1), creature);
            resolve(&mut single);
            cite!(["608.2i"]; assert_eq!(single.players[1].life, 17, "the original entry alone still counts after control changes"));
            enter(&mut g, 0);
        }
        cast(&mut g, Target::Player(1), creature);
        resolve(&mut g);
        cite!(["608.2i"]; case: format!("token={token}"); assert_eq!(g.players[1].life, 17));
    }
}
