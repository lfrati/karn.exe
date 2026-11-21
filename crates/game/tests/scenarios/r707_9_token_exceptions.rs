//! Token-copy exceptions modify copiable values on both faces, before ordinary entry.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone, Event};
use game::state::Answer;
fn card(name:&str,types:&str,text:&str,pt:Option<(i32,i32)>)->CardDef {CardDef::new(name,"{0}",types,text,pt).unwrap()}
fn duplication()->CardDef {
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
 let d=CardDef::from_face(faces.iter().find(|f|f.name=="Irenicus's Vile Duplication").unwrap()).unwrap();game::play::check(&d).unwrap();d
}
fn setup(model:CardDef)->Game {
 let defs=vec![duplication(),model,
 card("Duplicate","Instant","Create a token that's a copy of target creature.",None),
 card("Boost","Instant","Target creature gets +5/+5 until end of turn.",None),
 card("Strip","Instant","Target creature loses all abilities until end of turn.",None),
 card("Borrow","Instant","Target creature becomes a copy of target creature until end of turn.",None),
 card("Bear","Creature — Bear","",Some((2,2)))];
 for d in &defs{game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}
 let mut g=Game::new(defs,2,20);for _ in 0..8{g.put(0,6,Zone::Library);}g
}
fn model()->CardDef{card("Model","Legendary Creature — Wizard","Vigilance\n{0}: You gain 2 life.",Some((3,3)))}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
fn cast(g:&mut Game,index:usize,targets:&[ObjId],units:&str){
 let c=g.put(0,index,Zone::Hand);g.players[0].pool.colors[1]=4;
 g.act(0,paid(units,Move::Cast{card:c,targets:targets.iter().map(|o|vec![Target::Object(*o)]).collect()})).unwrap();pass(g);
}
fn token(g:&Game)->ObjId{*g.battlefield.iter().rev().find(|o|g.obj(**o).token()).unwrap()}
fn changed(g:&Game,o:ObjId){assert!(g.has(o,"Flying"));assert!(!g.obj(o).chars.supertypes.contains(&ir::Supertype::Legendary));}
#[test]
fn r707_9_full_oracle_targets_owned_creatures_and_exceptions_are_copiable(){
 let mut g=setup(model());let source=g.put(0,1,Zone::Battlefield);let opponent=g.put(1,1,Zone::Battlefield);
 let c=g.put(0,0,Zone::Hand);g.players[0].pool.colors[1]=4;
 cite!(["115.1"]; refused("an illegal target, Model: the target is one you control, and another player controls it (115.1)"): g.act(0,paid("UUUU", Move::Cast{card:c,targets:vec![vec![Target::Object(opponent)]]})));
 g.act(0,paid("UUUU", Move::Cast{card:c,targets:vec![vec![Target::Object(source)]]})).unwrap();pass(&mut g);let first=token(&g);changed(&g,first);cite!(["707.2"]; assert!(g.has(first,"Vigilance")));assert_eq!(g.battlefield.len(),3);
 assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Cast{..})).count(),1);assert_eq!(g.log.iter().filter(|e|matches!(e,Event::SpellCopied{..})).count(),0);assert_eq!(g.players[0].pool.total(),0);
 cast(&mut g,2,&[first],"");let second=token(&g);assert_ne!(first,second);changed(&g,second);
 g.move_object(source,Zone::Graveyard);g.move_object(first,Zone::Exile);
 g.act(0,Move::Activate{source:second,ability:1,targets:vec![],paying:vec![]}).unwrap();pass(&mut g);cite!(["707.2"]; assert_eq!(g.players[0].life,22));
}
#[test]
fn r707_2_exceptions_do_not_copy_counters_or_later_power_and_ability_layers(){
 let mut g=setup(model());let source=g.put(0,1,Zone::Battlefield);g.put_counters(source,"+1/+1",3);g.set_tapped(source,true);
 cast(&mut g,3,&[source],"");cast(&mut g,4,&[source],"");assert!(!g.has(source,"Vigilance"));assert_eq!(g.power(source),Some(11));
 cast(&mut g,0,&[source],"UUUU");let made=token(&g);changed(&g,made);assert!(g.has(made,"Vigilance"));assert!(!g.obj(made).tapped);cite!(["707.2"]; assert!(g.obj(made).counters.is_empty()));cite!(["707.2"]; assert_eq!(g.power(made),Some(3)));
}
#[test]
fn r707_8a_exceptions_change_both_token_faces_and_survive_source_retirement(){
 for back in [false,true]{
  let front=card("Front","Legendary Creature — Wizard","{0}: Transform this creature.",Some((2,2)));
  let rear=CardDef::new("Back","","Legendary Creature — Bird","{0}: Transform this creature.\nVigilance",Some((4,4))).unwrap();
  let mut g=setup(CardDef::transforming(front,rear).unwrap());let source=g.put(0,1,Zone::Battlefield);if back{assert!(g.transform(source,None));}
  cast(&mut g,0,&[source],"UUUU");let made=token(&g);cite!(["707.8a"]; case: back; assert_eq!(g.obj(made).transformed,back));changed(&g,made);
  g.move_object(source,Zone::Graveyard);g.retire_events(g.log.end());g.collect_departed();
  let mut clone=g.clone();for g in [&mut g,&mut clone]{
   for face in [!back,back]{g.act(0,Move::Activate{source:made,ability:0,targets:vec![],paying:vec![]}).unwrap();pass(g);assert_eq!(g.obj(made).transformed,face);changed(g,made);assert_eq!(g.power(made),Some(if face{4}else{2}));}
  }
 }
}
#[test]
fn r707_9_later_copy_layer_overrides_exceptions_then_expiration_restores_them(){
 let mut g=setup(model());let source=g.put(0,1,Zone::Battlefield);let bear=g.put(0,6,Zone::Battlefield);
 cast(&mut g,0,&[source],"UUUU");let made=token(&g);cast(&mut g,5,&[made,bear],"");cite!(["613.2"]; assert!(!g.has(made,"Flying")));cite!(["613.2"]; assert_eq!(g.power(made),Some(2)));
 let turn=g.turn;for _ in 0..30{if g.turn!=turn{break;}if matches!(g.decision,Some(game::state::Decision::Attackers{..})){g.act(g.active,Move::Attack(vec![])).unwrap();}else{pass(&mut g);}}
 assert_ne!(g.turn,turn);changed(&g,made);cite!(["514.2", "707.9b"]; assert_eq!(g.power(made),Some(3)));
}
#[test]
fn r707_6_suspended_token_entry_keeps_exceptions_and_makes_new_linked_choices(){
 use game::simulation::Simulation;
 let mut g=setup(card("Chooser","Legendary Creature — Wizard","As this creature enters, choose a color.\nWhen this creature enters, you gain 3 life.",Some((2,2))));
 let source=g.put(0,1,Zone::Battlefield);cast(&mut g,0,&[source],"UUUU");cite!(["707.6"]; assert!(g.decision.is_some()));cite!(["707.5"]; assert_eq!(g.battlefield,vec![source]));
 let mut sim=Simulation::new(g);let request=sim.request().unwrap().unwrap().clone();sim.snapshot(Some(0)).unwrap();let mut clone=sim.clone();
 for s in [&mut sim,&mut clone]{s.respond(request.id,request.player,Move::Choose(Answer::Value(game::binding::Value::Color(ir::Color::Red)))).unwrap();
  let g=s.game().unwrap();let made=token(g);changed(g,made);assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Created{..})).count(),1);
  for _ in 0..2{let r=s.request().unwrap().unwrap().clone();s.respond(r.id,r.player,Move::Pass).unwrap();}
  assert_eq!(s.game().unwrap().players[0].life,23);s.snapshot(None).unwrap();assert!(s.fault().is_none());
 }
}
#[test]
fn r707_9b_a_token_copy_except_it_is_1_1_has_that_base_power_and_toughness(){
 let shrink=card("Shrink Copy","Instant","Create a token that's a copy of target creature, except it's 1/1.",None);game::play::check(&shrink).unwrap();
 let mut g=Game::new(vec![shrink,model()],2,20);for _ in 0..8{g.put(0,1,Zone::Library);}
 let source=g.put(0,1,Zone::Battlefield);let c=g.put(0,0,Zone::Hand);
 g.act(0,Move::Cast{card:c,targets:vec![vec![Target::Object(source)]]}).unwrap();pass(&mut g);
 let made=token(&g);
 cite!(["707.9b"]; assert_eq!((g.power(made),g.toughness(made)),(Some(1),Some(1))));
 cite!(["707.9b", "707.2"]; assert!(g.has(made,"Vigilance")&&g.obj(made).chars.name=="Model"));
}
#[test]
fn r707_9_unimplemented_characteristic_replacement_exceptions_are_refused(){
 for text in ["Create a token that's a copy of target creature, except it has this ability."]{
  let d=card("Unsupported","Instant",text,None);
  crate::support::refused(game::play::check(&d).map_err(|e|e.0),"copy exception: characteristic replacement (707.9d)");
 }
}
