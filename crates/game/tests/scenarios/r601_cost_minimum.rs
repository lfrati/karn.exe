//! Direct total-cost adjustments happen after ordinary increases and reductions (601.2f).
use crate::support::{cite,paid};
use game::{CardDef,Game,Move,Zone,ObjId};
use game::legal::Step;
use game::announcement::CastCost;
use ir::Color;
use std::sync::OnceLock;
fn card(name:&str,cost:&str,kind:&str,text:&str)->CardDef {
    let d=CardDef::new(name,cost,kind,text,None).unwrap();game::play::check(&d).unwrap_or_else(|e|panic!("{name}: {}",e.0));d
}
fn sphere()->CardDef {
    static DEF:OnceLock<CardDef>=OnceLock::new();
    DEF.get_or_init(||{
        let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        let d=CardDef::from_face(faces.iter().find(|f|f.face_name=="Trinisphere").unwrap()).unwrap();game::play::check(&d).unwrap();d
    }).clone()
}
fn setup(cost:&str,extra:&str)->(Game,ObjId,ObjId) {
    let text=format!("{extra}You gain 1 life.");
    let mut g=Game::new(vec![sphere(),card("Spell",cost,"Instant",&text),
        card("Discount","{0}","Enchantment","Spells cost {9} less to cast."),
        card("Tapper","{0}","Artifact","Tap an untapped artifact you control: Add {C}{C}{C}."),
    ],2,20);
    let sphere=g.put(0,0,Zone::Battlefield);let spell=g.put(0,1,Zone::Hand);(g,sphere,spell)
}

#[test]
fn r601_2f_cost_floor_follows_reductions_without_replacing_colored_requirements() {
    let (mut g,_,spell)=setup("{4}{G}","");g.put(0,2,Zone::Battlefield);
    g.players[0].pool.colorless=3;assert!(!g.clone().act(g.priority, paid("CCG",Move::Cast { card: spell, targets: vec![] })).is_ok());
    g.players[0].pool.colors[Color::Green as usize]=1;g.players[0].pool.colorless=1;
    assert!(!g.clone().act(g.priority, paid("CCG",Move::Cast { card: spell, targets: vec![] })).is_ok());cite!(["601.2f"]; refused("your mana pool cannot pay that cost (118.3a): {2}{G} in total, {4} less than it was, raised to its minimum of {3} (601.2f)"): g.act(0,Move::Cast{card:spell,targets:vec![]}));
    g.players[0].pool.colorless=2;cite!(["601.2f"]; assert!(g.clone().act(g.priority, paid("CCG",Move::Cast { card: spell, targets: vec![] })).is_ok()));g.act(0,paid("CCG",Move::Cast{card:spell,targets:vec![]})).unwrap();
    cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(),0));
}
#[test]
fn r601_2f_tapped_floor_is_inactive_and_multiple_floors_do_not_add() {
    let (mut g,sphere,spell)=setup("{0}","");g.set_tapped(sphere,true);
    assert!(g.clone().act(g.priority, Move::Cast { card: spell, targets: vec![] }).is_ok());g.act(0,Move::Cast{card:spell,targets:vec![]}).unwrap();
    let (mut g,_,spell)=setup("{0}","");g.put(1,0,Zone::Battlefield);g.players[0].pool.colorless=3;
    assert!(g.clone().act(g.priority, paid("CCC",Move::Cast { card: spell, targets: vec![] })).is_ok());g.act(0,paid("CCC",Move::Cast{card:spell,targets:vec![]})).unwrap();cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(),0));
}
#[test]
fn r601_2f_alternative_cost_and_phyrexian_life_still_require_the_mana_floor() {
    let (mut g,_,spell)=setup("{6}","You may pay {0} rather than pay this spell's mana cost.\n");
    g.players[0].pool.colorless=2;let cast=Move::CastFor(CastCost::Alternative(0),Box::new(Move::Cast{card:spell,targets:vec![]}));
    cite!(["601.2f"]; refused("your mana pool cannot pay that cost (118.3a): {3} in total, raised to its minimum of {3} (601.2f)"): g.act(0,cast.clone()));g.players[0].pool.colorless=3;g.act(0,paid("CCC",cast)).unwrap();cite!(["118.9d", "601.2f"]; assert_eq!(g.players[0].pool.total(),0));
    let (mut g,_,spell)=setup("{G/P}","");g.players[0].pool.colorless=2;
    let cast=Move::WithLife(vec![true],Box::new(Move::Cast{card:spell,targets:vec![]}));
    cite!(["601.2f"]; refused("your mana pool cannot pay that cost (118.3a): {3} in total, raised to its minimum of {3} (601.2f)"): g.act(0,cast.clone()));g.players[0].pool.colorless=3;g.act(0,paid("CCC",cast)).unwrap();
    cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(),0));assert_eq!(g.players[0].life,18);
}
#[test]
fn r601_2f_two_hybrid_alternatives_get_their_own_final_mana_floor() {
    for (colorless, green) in [(0, 1), (2, 0)] {
        let (mut g, _, spell) = setup("{2/G}", "");
        g.put(0, 2, Zone::Battlefield);
        g.players[0].pool.colorless = colorless;
        g.players[0].pool.colors[4] = green;
        assert!(!g.clone().act(g.priority, paid(if green==1{"G"}else{"CC"},Move::Cast { card: spell, targets: vec![] })).is_ok(), "pool: C={colorless}, G={green}");
        let before = format!("{g:?}");
        cite!(["601.2f"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, paid(if green==1{"G"}else{"CC"},Move::Cast { card: spell, targets: vec![] })));
        assert_eq!(format!("{g:?}"), before);
    }
    for green in [false,true] {
        let (mut g,_,spell)=setup("{2/G}","");g.put(0,2,Zone::Battlefield);
        g.players[0].pool.colorless=if green{2}else{3};g.players[0].pool.colors[4]=u32::from(green);
        cite!(["601.2f"]; case: format!("green={green}"); assert!(g.clone().act(g.priority, paid(if green{"CCG"}else{"CCC"},Move::Cast { card: spell, targets: vec![] })).is_ok()));g.act(0,paid(if green{"CCG"}else{"CCC"},Move::Cast{card:spell,targets:vec![]})).unwrap();cite!(["601.2f"]; case: format!("green={green}"); assert_eq!(g.players[0].pool.total(),0));
    }
}
#[test]
fn r601_2f_floor_is_locked_before_its_source_is_tapped_during_mana_production() {
    let (mut g,sphere,spell)=setup("{0}","");let source=g.put(0,3,Zone::Battlefield);
    let plan=vec![Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source,ability:0,paying:vec![vec![sphere]],adding:vec![]}];
    g.act(0,Move::WithMana(plan,Box::new(paid("CCC",Move::Cast{card:spell,targets:vec![]})))).unwrap();
    assert!(g.obj(sphere).tapped);cite!(["601.2f", "601.2g"]; assert_eq!(g.players[0].pool.total(),0,"floor was locked while untapped"));
}
