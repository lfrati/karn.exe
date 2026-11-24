//! Equipment reuses attachment, targeting, layers and activation timing (301.5, 702.6).
use crate::support::{cite,paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Ability;

fn game(text: &str) -> Game {
    let defs = vec![
        CardDef::new("Harness", "{0}", "Artifact — Equipment", text, None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Reshape", "{0}", "Instant", "Target permanent becomes an artifact until end of turn.", None).unwrap(),
        CardDef::new("Animate", "{0}", "Instant", "Target artifact becomes a 2/2 artifact creature until end of turn.", None).unwrap(),
        CardDef::new("Transfer", "{0}", "Instant", "Attach target Equipment to target creature.", None).unwrap(),
        CardDef::new("Veil", "{0}", "Instant", "Target creature gains shroud until end of turn.", None).unwrap(),
        CardDef::new("Transmute", "{0}", "Instant", "Target permanent becomes an enchantment until end of turn.", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
    Game::new(defs, 3, 20)
}
fn resolve(g: &mut Game) { for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); } }
fn equip(g: &mut Game, source: ObjId, to: ObjId, units:Option<&str>) -> Result<(), String> {
    let ability = g.abilities(source).iter().position(|a| matches!(&**a, Ability::Activated { .. })).unwrap();
    let action=Move::Activate { source, ability, targets: vec![vec![Target::Object(to)]], paying: vec![] };
    g.act(0,units.map_or_else(||action.clone(),|units|paid(units,action.clone())))
}
fn cast(g: &mut Game, def: usize, to: ObjId) {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(to)]] }).unwrap();
    resolve(g);
}

#[test]
fn r301_5b_equipment_enters_unattached_and_equip_uses_the_stack() {
    let mut g = game("Equipped creature gets +1/+0.\nEquip {1}");
    let bear = g.put(0, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(&mut g);
    let equipment = *g.battlefield.last().unwrap();
    cite!(["301.5b"]; assert_eq!(g.attached_to(equipment), None));
    assert_eq!(g.power(bear), Some(2));
    let before = format!("{g:?}");
    cite!(["118.3a"]; refused("that mana is not in your mana pool (118.3a)"): equip(&mut g, equipment, bear,Some("C")));
    assert_eq!(format!("{g:?}"), before);
    g.players[0].pool.colorless = 1;
    equip(&mut g, equipment, bear,Some("C")).unwrap();
    assert_eq!(g.attached_to(equipment), None);
    cite!(["602.2"]; assert_eq!(g.stack.len(), 1));
    resolve(&mut g);
    cite!(["702.6a"]; assert_eq!(g.attached_to(equipment), Some(Target::Object(bear))));
    assert_eq!(g.power(bear), Some(3));
}

#[test]
fn r702_6a_equip_targets_only_your_creatures_at_sorcery_timing() {
    let mut g = game("Equip {0}");
    let equipment = g.put(0, 0, Zone::Battlefield);
    let mine = g.put(0, 1, Zone::Battlefield);
    let theirs = g.put(1, 1, Zone::Battlefield);
    let land = g.put(0, 2, Zone::Battlefield);
    for (wrong, why) in [
        (theirs, "an illegal target, Bear: the target is one you control, and another player controls it (115.1)"),
        (land, "an illegal target, Forest: it isn't a creature (115.1)"),
        (equipment, "an illegal target, Harness: it isn't a creature (115.1)"),
    ] {
        let before = format!("{g:?}");
        cite!(["702.6a"]; case: why; refused(why): equip(&mut g, equipment, wrong,None));
        assert_eq!(format!("{g:?}"), before);
    }
    equip(&mut g, equipment, mine,None).unwrap();
    // the stack is not empty
    cite!(["602.5d"]; refused("that ability is activated only as a sorcery: in a main phase of your own turn with the stack empty (602.5d)"): equip(&mut g, equipment, mine,None));
    resolve(&mut g);
    g.active = 1;
    // not your turn
    cite!(["602.5d"]; refused("that ability is activated only as a sorcery: in a main phase of your own turn with the stack empty (602.5d)"): equip(&mut g, equipment, mine,None));
}

#[test]
fn r301_5b_an_illegal_equip_target_leaves_the_previous_attachment_in_place() {
    for departed in [true, false] {
        let mut g = game("Equipped creature gets +1/+0.\nEquip {0}");
        let equipment = g.put(0, 0, Zone::Battlefield);
        let one = g.put(0, 1, Zone::Battlefield);
        let two = g.put(0, 1, Zone::Battlefield);
        equip(&mut g, equipment, one,None).unwrap(); resolve(&mut g);
        equip(&mut g, equipment, two,None).unwrap();
        if departed { g.destroy(two); } else { cast(&mut g, 6, two); }
        resolve(&mut g);
        cite!(["608.2b"]; case: format!("departed {departed}"); assert_eq!(g.attached_to(equipment), Some(Target::Object(one))));
        assert_eq!(g.power(one), Some(3));
    }
}

#[test]
fn r301_5d_other_effects_can_attach_equipment_to_an_opponents_creature() {
    let mut g = game("Equipped creature gets +1/+0.\nEquip {0}");
    let equipment = g.put(0, 0, Zone::Battlefield);
    let theirs = g.put(1, 1, Zone::Battlefield);
    let transfer = g.put(0, 5, Zone::Hand);
    g.act(0, Move::Cast { card: transfer, targets: vec![vec![Target::Object(equipment)], vec![Target::Object(theirs)]] }).unwrap();
    resolve(&mut g);
    cite!(["301.5b"]; assert_eq!(g.attached_to(equipment), Some(Target::Object(theirs))));
    cite!(["301.5d"]; assert_eq!((g.obj(equipment).controller, g.obj(theirs).controller), (0, 1)));
    assert_eq!(g.power(theirs), Some(3));
    g.state_based_actions();
    cite!(["301.5b", "301.5d"]; assert_eq!(g.attached_to(equipment), Some(Target::Object(theirs)), "control is not an ongoing attachment restriction"));
}

#[test]
fn r301_5c_illegal_equipment_detaches_but_stays_on_the_battlefield() {
    for cause in 0..4 {
        let mut g = game("Equipped creature gets +1/+0.\nEquip {0}");
        let equipment = g.put(0, 0, Zone::Battlefield);
        let bear = g.put(0, 1, Zone::Battlefield);
        equip(&mut g, equipment, bear,None).unwrap(); resolve(&mut g);
        match cause {
            0 => { g.destroy(bear); g.state_based_actions(); }
            1 => cast(&mut g, 3, bear),
            2 => cast(&mut g, 7, equipment),
            _ => cast(&mut g, 4, equipment),
        }
        cite!(["704.5n"]; case: format!("cause {cause}"); assert_eq!(g.obj(equipment).zone, Zone::Battlefield));
        cite!(["301.5c", "704.5n"]; case: format!("cause {cause}"); assert_eq!(g.attached_to(equipment), None, "cause {cause}"));
        assert!(g.obj(equipment).attached_to.is_none());
    }
}

#[test]
fn r701_3b_c_reattaching_equipment_moves_its_effect_and_only_changes_timestamp_when_needed() {
    let mut g = game("Equipped creature gets +1/+0.\nEquip {0}");
    let equipment = g.put(0, 0, Zone::Battlefield);
    let one = g.put(0, 1, Zone::Battlefield);
    let two = g.put(0, 1, Zone::Battlefield);
    assert!(g.attach(equipment, Target::Object(one)));
    let stamp = g.obj(equipment).stamp;
    for target in [Target::Object(one), Target::Object(equipment), Target::Player(0)] {
        cite!(["701.3b"]; case: format!("{target:?}"); assert!(!g.attach(equipment, target)));
        assert_eq!(g.obj(equipment).stamp, stamp);
    }
    assert!(g.attach(equipment, Target::Object(two)));
    cite!(["701.3c"]; assert!(g.obj(equipment).stamp > stamp));
    assert_eq!((g.power(one), g.power(two)), (Some(2), Some(3)));
}

#[test]
fn r603_10a_equipment_sees_the_equipped_creature_die_before_becoming_unattached() {
    let mut g = game("Equipped creature gets +1/+0.\nWhenever equipped creature dies, you gain 3 life.\nEquip {0}");
    let equipment = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Battlefield);
    equip(&mut g, equipment, bear,None).unwrap(); resolve(&mut g);
    g.destroy(bear);
    g.act(0, Move::Pass).unwrap();
    assert_eq!(g.attached_to(equipment), None);
    cite!(["603.10a"]; assert_eq!(g.stack.len(), 1));
    resolve(&mut g);
    cite!(["603.10a"]; assert_eq!(g.players[0].life, 23));
}

#[test]
fn r702_6d_multiple_equip_abilities_keep_their_own_costs() {
    let mut g = game("Equip {1}\nEquip {2}");
    let equipment = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Battlefield);
    for (mana, ability, accepted) in [(1, 2, true), (1, 3, false), (2, 2, true), (2, 3, true)] {
        let mut attempt = g.clone();
        attempt.players[0].pool.colorless = mana;
        let result = attempt.act(0, paid(if ability==2 {"C"}else if mana==1 {"C"}else{"CC"},Move::Activate { source: equipment, ability, targets: vec![vec![Target::Object(bear)]], paying: vec![] }));
        cite!(["702.6d", "118.3a"]; case: format!("{mana}/{ability}"); assert_eq!(result.is_ok(), accepted));
    }
}

#[test]
fn r400_7_an_equip_ability_does_not_attach_a_source_that_left_and_returned() {
    let mut g = game("Equipped creature gets +1/+0.\nEquip {0}");
    let equipment = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Battlefield);
    equip(&mut g, equipment, bear,None).unwrap();
    g.move_object(equipment, Zone::Hand);
    let card = g.players[0].hand[0];
    g.move_object(card, Zone::Battlefield);
    let returned = *g.battlefield.last().unwrap();
    resolve(&mut g);
    cite!(["400.7"]; assert_eq!(g.attached_to(returned), None));
    assert_eq!(g.power(bear), Some(2));
}

#[test]
fn r301_5b_nontargeting_attachment_ignores_shroud_while_equip_must_respect_it() {
    let mut g = game("Equip {0}");
    let equipment = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Battlefield);
    cast(&mut g, 6, bear);
    cite!(["702.18a"]; refused("an illegal target, Bear: it has shroud (702.18a)"): equip(&mut g, equipment, bear,None));
    assert!(g.attach(equipment, Target::Object(bear)));
    g.state_based_actions();
    cite!(["702.18a"]; assert_eq!(g.attached_to(equipment), Some(Target::Object(bear))));
}
