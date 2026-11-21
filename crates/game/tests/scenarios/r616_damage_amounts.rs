//! 614.1a/616.1: full pinned damage modifiers compose before damage results.
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, Target, Zone};
fn pinned(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d = CardDef::from_faces(CardDef::catalog(&faces).get(name).unwrap()).unwrap();
    game::play::check(&d).unwrap();
    d
}
fn def(name: &str, cost: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, cost, kind, text, pt).unwrap();
    game::play::check(&d).unwrap();
    d
}
fn choose(g: &mut Game, provider: game::ObjId) {
    let Some(Decision::Choose { player, choice: Choice::DamageReplacement { among, .. } }) = g.decision.clone() else {
        panic!("{:?}", g.decision)
    };
    let offer = among.iter().find(|o| o.effect.source == provider).unwrap().clone();
    g.act(player, Move::Choose(Answer::DamageReplacement(offer))).unwrap();
}
#[test]
fn r616_1_full_furnace_and_torbran_modify_spell_damage_in_affected_players_order() {
    for (first, expected) in [(0, 8), (1, 10)] {
        let mut g = Game::new(
            vec![pinned("Furnace of Rath"), pinned("Torbran, Thane of Red Fell"), pinned("Lightning Bolt")],
            2,
            20,
        );
        let providers = [g.put(0, 0, Zone::Battlefield), g.put(0, 1, Zone::Battlefield)];
        g.players[0].pool.colors = [10; 5];
        let spell = g.put(0, 2, Zone::Hand);
        g.act(0, paid("R", Move::Cast { card: spell, targets: vec![vec![Target::Player(1)]] })).unwrap();
        let spell_source = g.stack[0];
        for _ in 0..2 {
            g.act(g.priority, Move::Pass).unwrap();
        }
        assert!(g.resolving().is_some());
        assert_eq!(g.players[1].life, 20);
        choose(&mut g, providers[first]);
        assert!(g.resolving().is_none());
        cite!(["616.1e", "616.1f"]; case: format!("first={first}"); assert_eq!(g.players[1].life, 20 - expected));
        assert!(g.log.iter().any(|e|matches!(e,Event::DamageDealt{source,to:Target::Player(1),amount,..} if *source==spell_source&&*amount==expected as u32)));
    }
}
#[test]
fn r120_4_modified_damage_scales_infect_lifelink_but_not_toxic_value() {
    let source = def("Dealer", "{R}", "Creature", "Infect\nToxic 2\nLifelink", Some((3, 20)));
    let mut g = Game::new(vec![pinned("Furnace of Rath"), source], 2, 20);
    g.put(0, 0, Zone::Battlefield);
    let source = g.put(0, 1, Zone::Battlefield);
    // Public damage is noncombat: Infect scales, Toxic contributes only to combat.
    g.damage(source, Target::Player(1), 3);
    cite!(["702.90b"]; assert_eq!(g.players[1].life, 20));
    cite!(["120.3b", "702.164c"]; assert_eq!(g.counters(Target::Player(1)).get("poison"), Some(&6)));
    cite!(["120.3f"]; assert_eq!(g.players[0].life, 26));
}
#[test]
fn r614_5_two_furnaces_each_apply_once_and_torbran_preserves_friendly_recipients() {
    let source = def("Red", "{R}", "Creature", "", Some((3, 30)));
    let wall = def("Wall", "{0}", "Creature", "", Some((0, 30)));
    let mut g = Game::new(vec![pinned("Furnace of Rath"), pinned("Torbran, Thane of Red Fell"), source, wall], 2, 20);
    let furnace = g.put(0, 0, Zone::Battlefield);
    g.put(0, 0, Zone::Battlefield);
    g.put(0, 1, Zone::Battlefield);
    let source = g.put(0, 2, Zone::Battlefield);
    let wall = g.put(0, 3, Zone::Battlefield);
    g.damage(source, wall.into(), 3);
    choose(&mut g, furnace);
    cite!(["614.5", "616.1f"]; assert_eq!(g.obj(wall).damage, 12, "Torbran does not add to friendly permanent damage"));
}
#[test]
fn r614_1a_fight_uses_both_dealers_and_shared_modified_results() {
    let red = def("Red", "{R}", "Creature", "", Some((3, 30)));
    let wall = def("Wall", "{0}", "Creature", "", Some((2, 30)));
    let fight =
        def("Fight", "{0}", "Instant", "Target creature you control fights target creature you don't control.", None);
    let mut g = Game::new(vec![pinned("Furnace of Rath"), red, wall, fight], 2, 20);
    let furnace = g.put(0, 0, Zone::Battlefield);
    let red = g.put(0, 1, Zone::Battlefield);
    let wall = g.put(1, 2, Zone::Battlefield);
    let spell = g.put(0, 3, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![red.into()], vec![wall.into()]] }).unwrap();
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    // Each recipient has just one applicable effect, so no ordering question is needed.
    assert!(g.decision.is_none());
    cite!(["701.14a", "614.1a"]; assert_eq!(g.obj(red).damage, 4));
    cite!(["701.14a", "614.1a"]; assert_eq!(g.obj(wall).damage, 6));
    assert_ne!(red, furnace);
}

#[test]
fn r615_12_protection_is_filtered_once_but_unpreventable_damage_is_modified() {
    for unpreventable in [false, true] {
        let red = def("Red", "{R}", "Creature", "Lifelink", Some((3, 30)));
        let mut g = Game::new(
            vec![
                pinned("Furnace of Rath"),
                pinned("Torbran, Thane of Red Fell"),
                pinned("Kor Firewalker"),
                red,
                pinned("Leyline of Punishment"),
            ],
            2,
            20,
        );
        let furnace = g.put(0, 0, Zone::Battlefield);
        g.put(0, 1, Zone::Battlefield);
        let protected = g.put(1, 2, Zone::Battlefield);
        let source = g.put(0, 3, Zone::Battlefield);
        if unpreventable {
            g.put(0, 4, Zone::Battlefield);
        }
        let start = g.log.len();
        g.damage(source, protected.into(), 3);
        if unpreventable {
            choose(&mut g, furnace);
        }
        assert!(g.decision.is_none());
        cite!(["702.16e", "615.12"]; case: format!("unpreventable={unpreventable}"); assert_eq!(g.battlefield.contains(&protected), !unpreventable));
        // Leyline's "Players can't gain life." (101.2); protection prevents the damage, so no lifelink (702.16e)
        let rule = if unpreventable { "101.2" } else { "702.16e" };
        cite!([rule]; case: format!("unpreventable={unpreventable}"); assert_eq!(g.players[0].life, 20, "Leyline also stops lifelink; protected damage gives none"));
        let amounts: Vec<_> = g
            .log
            .iter()
            .skip(start)
            .filter_map(|e| if let Event::DamageDealt { amount, .. } = e { Some(*amount) } else { None })
            .collect();
        cite!(["702.16e", "615.12"]; case: format!("unpreventable={unpreventable}"); assert_eq!(amounts, if unpreventable { vec![8] } else { vec![] }));
    }
}
