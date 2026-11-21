//! 702.90/120.3: Infect changes damage results, not whether damage was dealt.
use game::state::Decision;
use game::{CardDef, Event, Game, Move, Target, Zone};
use std::sync::OnceLock;
use crate::support::cite;
fn pinned(name: &str) -> CardDef {
    static FACES: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    let faces = FACES.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    let d = CardDef::from_faces(CardDef::catalog(faces).get(name).unwrap()).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}
fn def(name: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, "{0}", kind, text, pt).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{text}: {}", e.0));
    d
}
fn resolve(g: &mut Game) {
    g.state_based_actions();
    for _ in 0..30 {
        if let Some(Decision::Order { player, abilities }) = &g.decision {
            g.act(*player, Move::Order((0..abilities.len()).collect())).unwrap();
            continue;
        }
        assert!(g.decision.is_none(), "{:?}", g.decision);
        if g.stack.is_empty() {
            return;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("resolution stalled")
}
#[test]
fn full_flensermite_combat_to_player_and_creature_is_damage_with_lifelink() {
    for blocked in [false, true] {
        let mut g = Game::new(vec![pinned("Flensermite"), def("Bear", "Creature", "", Some((2, 2)))], 2, 20);
        let source = g.put(0, 0, Zone::Battlefield);
        let target = g.put(1, 1, Zone::Battlefield);
        g.turn = 2;
        g.part = ir::Part::BeginningOfCombat;
        for _ in 0..2 {
            g.act(g.priority, Move::Pass).unwrap();
        }
        g.act(0, Move::Attack(vec![(source, Target::Player(1))])).unwrap();
        for _ in 0..20 {
            if let Some(Decision::Blockers { player, .. }) = g.decision.clone() {
                g.act(player, Move::Block(if blocked { vec![(target, source)] } else { vec![] })).unwrap();
                break;
            }
            g.act(g.priority, Move::Pass).unwrap();
        }
        for _ in 0..10 {
            if g.log.iter().any(|e| matches!(e,Event::DamageDealt{source:s,combat:true,..}if *s==source)) {
                break;
            }
            g.act(g.priority, Move::Pass).unwrap();
        }
        cite!(["120.3f"]; case: format!("blocked {blocked}"); assert_eq!(g.players[0].life, 21));
        cite!(["702.90b"]; case: format!("blocked {blocked}"); assert_eq!(g.players[1].life, 20));
        if blocked {
            cite!(["702.90c", "120.3d"]; assert_eq!(g.counters(target)["-1/-1"], 1));
            cite!(["702.90c"]; assert_eq!(g.obj(target).damage, 0));
            assert_eq!(g.power(target), Some(1));
            assert_eq!(g.obj(source).zone, Zone::Gone);
        } else {
            cite!(["702.90b", "120.3b"]; assert_eq!(g.counters(Target::Player(1))["poison"], 1));
        }
    }
}
#[test]
fn full_mamba_regenerates_marked_lethal_damage_but_not_infect_zero_toughness() {
    let mut g = Game::new(
        vec![pinned("Blight Mamba"), pinned("Flensermite"), def("Bear", "Creature", "", Some((2, 2)))],
        2,
        20,
    );
    g.players[0].pool.colors = [8; 5];
    let mamba = g.put(0, 0, Zone::Battlefield);
    let inf = g.put(1, 1, Zone::Battlefield);
    let ordinary = g.put(1, 2, Zone::Battlefield);
    for source in [ordinary, inf] {
        g.act(0, crate::support::paid("GG",Move::Activate { source: mamba, ability: 1, targets: vec![], paying: vec![] })).unwrap();
        resolve(&mut g);
        g.damage(source, mamba.into(), 1);
        resolve(&mut g);
        if source == ordinary {
            assert_eq!(g.obj(mamba).zone, Zone::Battlefield);
            assert!(g.obj(mamba).tapped);
            cite!(["701.19a"]; assert_eq!(g.obj(mamba).damage, 0));
        } else {
            cite!(["704.5f", "702.90c"]; assert_eq!(g.obj(mamba).zone, Zone::Gone));
            assert!(
                g.log
                    .iter()
                    .any(|e| matches!(e,Event::CountersPut{on,kind,count:1,..}if *on==mamba.into()&&kind=="-1/-1"))
            );
        }
    }
}
#[test]
fn full_tainted_strike_grants_infect_and_ability_removal_restores_life_loss() {
    let mut g = Game::new(
        vec![
            pinned("Tainted Strike"),
            def("Bear", "Creature", "", Some((2, 4))),
            def("Erase", "Instant", "Target creature loses all abilities until end of turn.", None),
        ],
        2,
        20,
    );
    g.players[0].pool.colors = [8; 5];
    let bear = g.put(0, 1, Zone::Battlefield);
    for (card, expected) in [(0, true), (2, false)] {
        let spell = g.put(0, card, Zone::Hand);
        g.act(0, crate::support::paid(if card == 0 { "B" } else { "" },Move::Cast { card: spell, targets: vec![vec![bear.into()]] })).unwrap();
        resolve(&mut g);
        cite!(["613.1f"]; case: format!("card {card}"); assert_eq!(g.seen(bear, None).has(ir::Keyword::from_name("Infect").unwrap()), expected));
        g.damage(bear, Target::Player(1), 3);
        assert_eq!(g.counters(Target::Player(1))["poison"], 3);
        cite!(["702.90b", "120.3a"]; case: format!("card {card}"); assert_eq!(g.players[1].life, if expected { 20 } else { 17 }));
    }
}
#[test]
fn infect_uses_departed_source_lki_and_effective_controller_not_owner() {
    let source = def(
        "Martyr",
        "Creature",
        "Infect\nLifelink\nSacrifice this creature: This creature deals 2 damage to target player.",
        Some((1, 1)),
    );
    let mut g = Game::new(vec![source, def("Control", "Instant", "Gain control of target creature.", None)], 3, 20);
    let old = g.put(0, 0, Zone::Battlefield);
    let control = g.put(2, 1, Zone::Hand);
    g.priority = 2;
    g.act(2, Move::Cast { card: control, targets: vec![vec![old.into()]] }).unwrap();
    resolve(&mut g);
    assert_eq!(g.controller(old), 2);
    g.priority = 2;
    g.act(2, Move::Activate { source: old, ability: 2, targets: vec![vec![Target::Player(1)]], paying: vec![] })
        .unwrap();
    assert_eq!(g.obj(old).zone, Zone::Gone);
    let successor = g.players[0].graveyard[0];
    g.move_object(successor, Zone::Battlefield);
    resolve(&mut g);
    cite!(["702.90d"]; assert_eq!(g.players[1].life, 20));
    cite!(["702.90d", "120.3b"]; assert_eq!(g.counters(Target::Player(1))["poison"], 2));
    cite!(["702.15c", "120.3f"]; assert_eq!((g.players[0].life, g.players[2].life), (20, 22)));
    assert!(g.log.iter().any(|e| matches!(e,Event::DamageDealt{source,..}if *source==old)));
}
#[test]
fn infect_from_any_zone_and_zero_or_protected_hits_use_the_existing_damage_path() {
    let mut g = Game::new(
        vec![pinned("Flensermite"), def("Protected", "Creature", "Protection from black", Some((2, 2)))],
        2,
        20,
    );
    let target = g.put(1, 1, Zone::Battlefield);
    for zone in [Zone::Battlefield, Zone::Hand, Zone::Graveyard, Zone::Exile, Zone::Library] {
        let source = g.put(0, 0, zone);
        let from = g.log.len();
        let life = g.players[0].life;
        g.damage(source, target.into(), 1);
        g.damage(source, Target::Player(1), 0);
        assert_eq!(g.players[0].life, life);
        cite!(["702.16e", "120.8"]; case: format!("{zone:?}"); assert!(!g.log.iter().skip(from).any(|e| matches!(e, Event::DamageDealt { .. } | Event::CountersPut { .. }))));
        g.damage(source, Target::Player(1), 1);
        assert_eq!(g.players[0].life, life + 1);
        cite!(["702.90e"]; case: format!("{zone:?}"); assert_eq!(g.players[1].life, 20));
    }
    cite!(["702.90e"]; assert_eq!(g.counters(Target::Player(1))["poison"], 5));
    cite!(["702.16e"]; assert!(g.counters(target).is_empty()));
    assert_eq!(g.obj(target).damage, 0);
}
