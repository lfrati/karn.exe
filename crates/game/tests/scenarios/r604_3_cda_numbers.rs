//! CR 604.3/208.2: a characteristic definition can reuse its own preceding quantity.
use crate::support::cite;
use game::{CardDef,Game,Move,Target,Zone};
use game::simulation::Simulation;
fn barrow()->CardDef{
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
 let d=CardDef::from_face(faces.iter().find(|f|f.name=="Barrowgoyf").unwrap()).unwrap();
 game::play::check(&d).unwrap();d
}
fn defs()->Vec<CardDef>{vec![barrow(),
 CardDef::new("Land","","Land","",None).unwrap(),
 CardDef::new("Instant","{0}","Instant","You gain 1 life.",None).unwrap(),
 CardDef::new("Dual","{0}","Artifact Creature — Bear","",Some((2,2))).unwrap(),
 CardDef::new("Erase","{0}","Instant","Target creature loses all abilities until end of turn.",None).unwrap(),
]}
fn respond(sim:&mut Simulation,m:Move){let r=sim.request().unwrap().unwrap().clone();sim.respond(r.id,r.player,m).unwrap();}
fn passes(sim:&mut Simulation){for _ in 0..sim.game().unwrap().players.len(){respond(sim,Move::Pass);}}
#[test]
fn r604_3_count_distinct_types_in_every_graveyard_from_every_zone(){
 let mut g=Game::new(defs(),3,20);let objects=[Zone::Hand,Zone::Library,Zone::Graveyard,Zone::Exile,Zone::Battlefield].map(|z|g.put(0,0,z));
 let dual=g.put(2,3,Zone::Graveyard);g.put(1,1,Zone::Graveyard);g.put(1,2,Zone::Graveyard);g.put(2,2,Zone::Graveyard);
 for o in objects{cite!(["604.3","208.2a"];case:format!("{:?}",g.obj(o).zone);assert_eq!((g.power(o),g.toughness(o)),(Some(4),Some(5))));}
 g.move_object(dual,Zone::Exile);
 for o in objects{cite!(["604.3","208.2a"];case:format!("{:?}",g.obj(o).zone);assert_eq!((g.power(o),g.toughness(o)),(Some(3),Some(4))));}
}
#[test]
fn r208_2a_removing_the_definition_leaves_the_printed_star_offset(){
 let mut g=Game::new(defs(),2,20);let goyf=g.put(0,0,Zone::Battlefield);g.put(1,3,Zone::Graveyard);let erase=g.put(0,4,Zone::Hand);
 let mut sim=Simulation::new(g);respond(&mut sim,Move::Cast{card:erase,targets:vec![vec![Target::Object(goyf)]]});passes(&mut sim);
 cite!(["613.1f"];assert_eq!((sim.game().unwrap().power(goyf),sim.game().unwrap().toughness(goyf)),(Some(0),Some(1))));
}
#[test]
fn r603_2_barrowgoyf_combat_mill_and_optional_return_share_the_actual_milled_group(){
 for (mill,keep) in [(false,false),(true,false),(true,true)]{
  let mut g=Game::new(defs(),2,20);let goyf=g.put(0,0,Zone::Battlefield);let old=g.put(0,0,Zone::Graveyard);g.put(1,1,Zone::Graveyard);g.put(1,2,Zone::Graveyard);
  let milled:Vec<_>=[0,0,1].into_iter().map(|d|g.put(0,d,Zone::Library)).collect();
  let mut sim=Simulation::new(g);passes(&mut sim);passes(&mut sim);respond(&mut sim,Move::Attack(vec![(goyf,Target::Player(1))]));
  for _ in 0..20{
   match sim.request().unwrap().unwrap().question{game::legal::Asked::May=>break,game::legal::Asked::Blockers{..}=>respond(&mut sim,Move::Block(vec![])),_=>respond(&mut sim,Move::Pass)}
  }
  cite!(["603.2"];case:format!("mill={mill}, keep={keep}");assert!(matches!(sim.request().unwrap().unwrap().question,game::legal::Asked::May)));cite!(["702.15b"];case:format!("mill={mill}, keep={keep}");assert_eq!(sim.game().unwrap().players[0].life,23));assert_eq!(sim.game().unwrap().players[1].life,17);
  respond(&mut sim,Move::Choose(if mill{game::state::Answer::Yes}else{game::state::Answer::No}));
  if mill{
   // The owner puts the three physical library cards into their graveyard in this order (404.3).
   respond(&mut sim,Move::Choose(game::state::Answer::Objects(milled)));
   cite!(["701.17a"];case:format!("keep={keep}");assert!(sim.game().unwrap().players[0].library.is_empty()));
   assert!(matches!(sim.request().unwrap().unwrap().question,game::legal::Asked::May));
   respond(&mut sim,Move::Choose(if keep{game::state::Answer::Yes}else{game::state::Answer::No}));
   if keep{
    let r=sim.request().unwrap().unwrap().clone();let game::legal::Asked::Objects{among,count:1,..}=&r.question else{panic!("creature selection")};assert_eq!(among.len(),2);assert!(!among.contains(&old));
    crate::support::refused(sim.respond(r.id,r.player,Move::Choose(game::state::Answer::Objects(vec![old]))), "a chosen object is not among the offered objects to return (608.2d)");
    let selected=among[0];let mut cloned=sim.clone();for sim in [&mut sim,&mut cloned]{respond(sim,Move::Choose(game::state::Answer::Objects(vec![selected])));assert_eq!(sim.game().unwrap().players[0].hand.len(),1);}
   }
  }else{assert_eq!(sim.game().unwrap().players[0].library.len(),3);}
  assert_eq!(sim.game().unwrap().players[0].hand.len(),usize::from(keep));
 }
}
#[test]
fn r603_2_noncombat_damage_does_not_trigger_barrowgoyf_but_still_has_lifelink(){
 let mut g=Game::new(defs(),2,20);let goyf=g.put(0,0,Zone::Battlefield);g.damage(goyf,Target::Player(1),2);let mut sim=Simulation::new(g);
 let _=sim.request().unwrap();cite!(["603.2"];assert!(sim.game().unwrap().stack.is_empty()));cite!(["702.15b"];assert_eq!(sim.game().unwrap().players[0].life,22));
}
#[test]
fn r604_3_an_unbound_recalled_number_remains_refused(){
 let d=CardDef::new("Unbound","{0}","Creature — Bear","This creature's toughness is equal to that number plus 1.",Some((0,1))).unwrap();
 crate::support::refused(game::play::check(&d).map_err(|e|e.0),"that number: no preceding numeric producer");
}
