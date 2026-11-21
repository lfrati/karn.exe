//! Integration regression for frozen simultaneous sacrifice eligibility.
use crate::support::{cite, paid};
use game::{CardDef, Event, Game, Move, Target, Zone};
fn card(name:&str,kind:&str,text:&str,pt:Option<(i32,i32)>)->CardDef {
    let d=CardDef::new(name,"{0}",kind,text,pt).unwrap();game::play::check(&d).unwrap();d
}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
fn cast(g:&mut Game,def:usize,spending:&str){
    let c=g.put(0,def,Zone::Hand);g.players[0].pool.colors=[10;5];
    g.act(0,paid(spending,Move::Cast{card:c,targets:vec![]})).unwrap();pass(g);
}
fn setup()->(Game,game::ObjId,game::ObjId){
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let thief=CardDef::from_face(faces.iter().find(|f|f.name=="Master Thief").unwrap()).unwrap();game::play::check(&thief).unwrap();
    let mut g=Game::new(vec![thief,
        card("Relic","Artifact Creature — Golem","",Some((2,2))),
        card("Sacrifice","Instant","Sacrifice all creatures you control.",None),
        card("Altar","Artifact","Sacrifice two creatures: You gain 3 life.",None)],2,20);
    let relic=g.put(1,1,Zone::Battlefield);cast(&mut g,0, "WWUU");
    let source=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(0)).unwrap();
    g.act(0,Move::ChooseTargets(vec![vec![Target::Object(relic)]])).unwrap();pass(&mut g);
    assert_eq!(g.controller(relic),0);
    // Source-first mutation is the case that used to invalidate the second sacrifice.
    g.battlefield.retain(|o|*o!=source);g.battlefield.insert(0,source);
    (g,source,relic)
}
fn assert_sacrificed(g:&Game,source:game::ObjId,relic:game::ObjId){
    assert!(!g.battlefield.contains(&source)&&!g.battlefield.contains(&relic));
    let events=g.log.iter().filter_map(|e|match e{Event::Sacrificed{player,object}=>Some((*player,*object)),_=>None}).collect::<Vec<_>>();
    cite!(["701.21a"];assert_eq!(events,vec![(0,source),(0,relic)]));
    assert_eq!(g.players[1].graveyard.len(),1);
    cite!(["701.21a"];assert_eq!(g.obj(g.players[1].graveyard[0]).card,Some(1)));
}
#[test]
fn r701_21_simultaneous_sacrifice_freezes_control_before_full_master_thief_departs(){
    let (mut g,source,relic)=setup();cast(&mut g,2, "");assert_sacrificed(&g,source,relic);
}
#[test]
fn r601_2h_simultaneous_sacrifice_cost_freezes_control_before_full_master_thief_departs(){
    let (mut g,source,relic)=setup();let altar=g.put(0,3,Zone::Battlefield);
    cite!(["601.2h"];ok:g.act(0,Move::Activate{source:altar,ability:0,targets:vec![],paying:vec![vec![source,relic]]}));
    assert_sacrificed(&g,source,relic);pass(&mut g);assert_eq!(g.players[0].life,23);
}
