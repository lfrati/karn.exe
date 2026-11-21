//! Static damage prohibitions and recipient-dependent simultaneous damage (615.12, 120.4).
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone};
fn setup() -> Game {
    let defs=vec![
        CardDef::new("Sunspine Lynx","{2}{R}{R}","Creature — Elemental Cat","Players can't gain life.\nDamage can't be prevented.\nWhen this creature enters, it deals damage to each player equal to the number of nonbasic lands that player controls.",Some((5,4))).unwrap(),
        CardDef::new("Land","","Land","{T}: Add {C}.",None).unwrap(),
        CardDef::new("Mountain","","Basic Land — Mountain","",None).unwrap(),
        CardDef::new("Guard","{W}","Creature — Soldier","Protection from red",Some((2,4))).unwrap(),
        CardDef::new("Remove abilities","{0}","Instant","Target creature loses all abilities until end of turn.",None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    Game::new(defs, 3, 20)
}
fn resolve(g: &mut Game) {
    for _ in 0..3 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
#[test]
fn r120_4_damage_is_counted_for_each_player_before_any_is_dealt() {
    let mut g = setup();
    for player in 0..3 {
        for _ in 0..player {
            g.put(player, 1, Zone::Battlefield);
        }
        g.put(player, 2, Zone::Battlefield);
    }
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[ir::Color::Red as usize] = 4;
    g.act(0, paid("RRRR", Move::Cast { card, targets: vec![] })).unwrap();
    resolve(&mut g);
    resolve(&mut g);
    cite!(["120.3a"]; assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![20, 19, 18]));
    for p in 0..3 {
        g.gain_life(p, 5).unwrap();
    }
    cite!(["101.2"]; assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![20, 19, 18]));
}
#[test]
fn r615_12_static_unpreventable_ends_when_its_source_leaves() {
    let mut g = setup();
    let lynx = g.put(0, 0, Zone::Battlefield);
    let guard = g.put(1, 3, Zone::Battlefield);
    assert!(g.unpreventable());
    g.damage(lynx, Target::Object(guard), 1);
    cite!(["615.12"]; assert_eq!(g.obj(guard).damage, 1));
    cite!(["702.16b"]; assert!(!g.can_target(&game::select::Ctx::new(0, lynx), Target::Object(guard))));
    g.move_object(lynx, Zone::Graveyard);
    cite!(["611.3b"]; assert!(!g.unpreventable()));
    g.damage(lynx, Target::Object(guard), 1);
    cite!(["611.3b", "702.16e"]; assert_eq!(g.obj(guard).damage, 1));
    g.gain_life(1, 1).unwrap();
    cite!(["611.3b"]; assert_eq!(g.players[1].life, 21));
}
#[test]
fn r613_1f_ability_removal_suppresses_both_global_rules() {
    let mut g = setup();
    let lynx = g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 4, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(lynx)]] }).unwrap();
    resolve(&mut g);
    cite!(["613.1f"]; assert!(!g.unpreventable()));
    g.gain_life(2, 1).unwrap();
    cite!(["613.1f"]; assert_eq!(g.players[2].life, 21));
}
