//! Frantic Search, Rewind and Unwind select lands on resolution, then use ordinary untap.
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};
use std::sync::OnceLock;
fn definitions()->Vec<CardDef>{
 static DEFS:OnceLock<Vec<CardDef>>=OnceLock::new();DEFS.get_or_init(||{
  let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
  let mut ds:Vec<_>=["Frantic Search","Rewind","Unwind"].iter().map(|name|CardDef::from_face(faces.iter().find(|f|&f.name==name).unwrap()).unwrap()).collect();
  for (name,types,text,pt) in [("Island","Basic Land — Island","",None),("Forest","Basic Land — Forest","",None),
   ("Bear","Creature — Bear","",Some((2,2))),("Witness","Instant","You gain 1 life.",None),
   ("Unstoppable","Instant","This spell can't be countered.\nYou gain 1 life.",None),
   ("Duplicate","Instant","Create a token that's a copy of target land.",None)] {
   ds.push(CardDef::new(name,"{0}",types,text,pt).unwrap());
  }
  for d in &ds{game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}ds
 }).clone()
}
fn setup()->Game{let mut g=Game::new(definitions(),2,20);for p in 0..2{for _ in 0..10{g.put(p,3,Zone::Library);}}g}
fn pass(g:&mut Game){g.act(g.priority,Move::Pass).unwrap();g.act(g.priority,Move::Pass).unwrap();}
fn lands(g:&mut Game,n:usize)->Vec<ObjId>{(0..n).map(|i|{let o=g.put(i%2,3+i%2,Zone::Battlefield);g.set_tapped(o,true);o}).collect()}
fn cast(g:&mut Game,index:usize,targets:Vec<Vec<Target>>)->ObjId{let c=g.put(0,index,Zone::Hand);g.players[0].pool.colors[1]=4;g.act(0,crate::support::paid(["UUU", "UUUU", "UUU", "", "", "", "", "", ""][index], Move::Cast{card:c,targets})).unwrap();c}
fn selected(g:&Game)->(&[ObjId],usize){match &g.decision{Some(Decision::Choose{player:0,choice:Choice::Select{among,min:0,max}})=>(among,*max),other=>panic!("expected range selection: {other:?}")}}
fn choose(g:&mut Game,ids:Vec<ObjId>){g.act(0,Move::Choose(Answer::Objects(ids))).unwrap();}
fn discard_order(g: &mut Game, discarded: Vec<ObjId>) {
 assert_eq!(g.decision,Some(Decision::Choose{player:0,choice:Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:discarded.clone()}}));
 g.act(0,Move::Choose(Answer::Objects(vec![discarded[1],discarded[0]]))).unwrap();
}
fn witness(g:&mut Game,index:usize)->ObjId{
 g.act(0,Move::Pass).unwrap();let card=g.put(1,index,Zone::Hand);g.act(1,Move::Cast{card,targets:vec![]}).unwrap();let spell=*g.stack.last().unwrap();g.act(1,Move::Pass).unwrap();spell
}
#[test]
fn r608_2_frantic_search_draws_then_discards_before_choosing_any_players_lands(){
 let mut g=setup();let ls=lands(&mut g,5);let held=g.put(0,5,Zone::Hand);cast(&mut g,0,vec![]);pass(&mut g);
 assert_eq!(g.players[0].hand.len(),3);assert_eq!(g.players[0].library.len(),8);assert!(ls.iter().all(|o|g.obj(*o).tapped));
 cite!(["608.2c"]; assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::Objects{verb:ir::Verb::Discard,count:2,..},..}))));
 let discard: Vec<_>=g.players[0].hand.iter().copied().filter(|o|*o!=held).collect();choose(&mut g,discard.clone());discard_order(&mut g,discard);
 assert_eq!(g.players[0].hand,vec![held]);assert_eq!(g.players[0].graveyard.len(),2);let (among,max)=selected(&g);assert_eq!(max,3);assert_eq!(among.len(),5);assert!(among.contains(&ls[1]));
 choose(&mut g,vec![ls[1],ls[3]]);cite!(["701.26b"]; assert!(!g.obj(ls[1]).tapped));assert!(!g.obj(ls[3]).tapped);cite!(["608.2d"]; assert!(g.obj(ls[0]).tapped));assert!(g.stack.is_empty());assert_eq!(g.players[0].graveyard.len(),3);
}
#[test]
fn r608_2d_bounded_selection_allows_zero_some_or_maximum_without_targeting(){
 for n in [0,1,3]{let mut g=setup();let ls=lands(&mut g,5);let drawn=g.top_cards(0,2);cast(&mut g,0,vec![]);pass(&mut g);let discarded=g.players[0].hand.clone();assert_eq!(discarded.iter().map(|id|g.obj(*id).card).collect::<Vec<_>>(),drawn.iter().map(|id|g.obj(*id).card).collect::<Vec<_>>());discard_order(&mut g,discarded);selected(&g);choose(&mut g,ls[..n].to_vec());
  cite!(["608.2d"]; case: n; assert_eq!(ls.iter().filter(|o|!g.obj(**o).tapped).count(),n));assert_eq!(g.players[0].hand.len(),0);assert_eq!(g.players[0].library.len(),8);
 }
 let mut g=setup();let drawn=g.top_cards(0,2);cast(&mut g,0,vec![]);pass(&mut g);let discarded=g.players[0].hand.clone();assert_eq!(discarded.iter().map(|id|g.obj(*id).card).collect::<Vec<_>>(),drawn.iter().map(|id|g.obj(*id).card).collect::<Vec<_>>());discard_order(&mut g,discarded);assert!(g.decision.is_none());assert!(g.stack.is_empty());
}
#[test]
fn r701_5_rewind_and_unwind_counter_before_untap_and_limit_selection(){
 for (index,max) in [(1,4),(2,3)]{let mut g=setup();let ls=lands(&mut g,5);let spell=witness(&mut g,6);cast(&mut g,index,vec![vec![Target::Object(spell)]]);pass(&mut g);
  cite!(["701.6a"]; case: index; assert_eq!(g.players[1].graveyard.len(),1));assert!(!g.stack.contains(&spell));cite!(["608.2d"]; case: index; assert_eq!(selected(&g).1,max));cite!(["608.2c"]; case: index; assert!(ls.iter().all(|o|g.obj(*o).tapped)));choose(&mut g,ls[..max].to_vec());assert!(g.stack.is_empty());cite!(["701.26b"]; case: index; assert_eq!(ls.iter().filter(|o|!g.obj(**o).tapped).count(),max));
 }
 let mut g=setup();let creature=g.put(1,5,Zone::Stack);let c=g.put(0,2,Zone::Hand);g.players[0].pool.colors[1]=3;cite!(["115.1"]; refused("an illegal target, Bear: it is a creature (115.1)"): g.act(0,Move::Cast{card:c,targets:vec![vec![Target::Object(creature)]]}));
}
#[test]
fn r608_2b_missing_target_fizzles_but_uncounterable_target_still_allows_untapping(){
 for index in [1,2]{
  let mut g=setup();let ls=lands(&mut g,1);let spell=witness(&mut g,6);cast(&mut g,index,vec![vec![Target::Object(spell)]]);assert!(g.counter(spell));pass(&mut g);cite!(["608.2b"]; case: index; assert!(g.decision.is_none()));cite!(["608.2b"]; case: index; assert!(g.obj(ls[0]).tapped));assert!(g.stack.is_empty());
  let mut g=setup();let ls=lands(&mut g,1);let spell=witness(&mut g,7);cast(&mut g,index,vec![vec![Target::Object(spell)]]);pass(&mut g);assert!(g.stack.contains(&spell));assert_eq!(selected(&g).1,1);choose(&mut g,ls.clone());cite!(["608.2b"]; case: index; assert!(!g.obj(ls[0]).tapped));pass(&mut g);assert_eq!(g.players[1].life,21);
 }
}
#[test]
fn r122_1d_selected_untaps_use_stun_replacement_and_refuse_already_untapped_lands(){
 let mut g=setup();let ls=lands(&mut g,3);g.put_counters(ls[0],"stun",2);g.put_counters(ls[1],"stun",2);g.set_tapped(ls[1],false);
 let drawn=g.top_cards(0,2);cast(&mut g,0,vec![]);pass(&mut g);let discarded=g.players[0].hand.clone();assert_eq!(discarded.iter().map(|id|g.obj(*id).card).collect::<Vec<_>>(),drawn.iter().map(|id|g.obj(*id).card).collect::<Vec<_>>());discard_order(&mut g,discarded);assert!(!selected(&g).0.contains(&ls[1]));cite!(["608.2d", "701.26b"]; refused("a chosen object is not among the offered objects (608.2d): choose between 0 and 2 distinct matching objects"): g.act(0,Move::Choose(Answer::Objects(vec![ls[1]]))));choose(&mut g,vec![ls[0],ls[2]]);cite!(["122.1d"]; assert!(g.obj(ls[0]).tapped));cite!(["122.1d"]; assert_eq!(g.obj(ls[0]).counters["stun"],1));assert!(!g.obj(ls[1]).tapped);assert_eq!(g.obj(ls[1]).counters["stun"],2);cite!(["701.26b"]; assert!(!g.obj(ls[2]).tapped));
}
#[test]
fn r111_6_token_lands_are_selectable_permanents(){
 let mut g=setup();let land=g.put(0,3,Zone::Battlefield);cast(&mut g,8,vec![vec![Target::Object(land)]]);pass(&mut g);let token=*g.battlefield.last().unwrap();assert!(g.obj(token).token());g.set_tapped(token,true);
 let drawn=g.top_cards(0,2);cast(&mut g,0,vec![]);pass(&mut g);let discarded=g.players[0].hand.clone();assert_eq!(discarded.iter().map(|id|g.obj(*id).card).collect::<Vec<_>>(),drawn.iter().map(|id|g.obj(*id).card).collect::<Vec<_>>());discard_order(&mut g,discarded);cite!(["111.6"]; assert!(selected(&g).0.contains(&token)));choose(&mut g,vec![token]);cite!(["701.26b"]; assert!(!g.obj(token).tapped));
}
#[test]
fn r608_2d_managed_choice_rejects_wrong_duplicate_excess_and_stale_answers_and_resumes_clones_once(){
 use game::simulation::{ResponseError,Simulation};
 let mut g=setup();let ls=lands(&mut g,5);let nonland=g.put(0,5,Zone::Battlefield);let stale=g.put(0,3,Zone::Battlefield);g.move_object(stale,Zone::Graveyard);
 let drawn=g.top_cards(0,2);cast(&mut g,0,vec![]);pass(&mut g);let discarded=g.players[0].hand.clone();assert_eq!(discarded.iter().map(|id|g.obj(*id).card).collect::<Vec<_>>(),drawn.iter().map(|id|g.obj(*id).card).collect::<Vec<_>>());discard_order(&mut g,discarded);selected(&g);let mut sim=Simulation::new(g);let r=sim.request().unwrap().unwrap().clone();sim.snapshot(Some(0)).unwrap();
 assert_eq!(sim.respond(r.id,1,Move::Choose(Answer::Objects(vec![]))),Err(ResponseError::WrongPlayer{expected:r.player,received:1}));
 // each answer breaks one constraint (kind, match, gone, duplicate, count)
 let range=": choose between 0 and 3 distinct matching objects";
 for (answer,why) in [(Answer::Yes,"that answer chooses no objects (608.2d)"),(Answer::Objects(vec![nonland]),"a chosen object is not among the offered objects (608.2d)"),(Answer::Objects(vec![stale]),"a chosen object is not among the offered objects (608.2d)"),(Answer::Objects(vec![ls[0];2]),"each object is chosen once (608.2d)"),(Answer::Objects(ls[..4].to_vec()),"4 chosen (608.2d)")]{let why=format!("{why}{range}");cite!(["608.2d"]; case: &why; refused(&why): sim.respond(r.id,r.player,Move::Choose(answer)));assert_eq!(sim.request().unwrap().unwrap().id,r.id);}
 let mut clone=sim.clone();for s in [&mut sim,&mut clone]{s.respond(r.id,r.player,Move::Choose(Answer::Objects(vec![ls[0],ls[1]]))).unwrap();assert!(matches!(s.respond(r.id,r.player,Move::Choose(Answer::Objects(vec![]))),Err(ResponseError::Stale{received,..}) if received==r.id));let g=s.game().unwrap();assert_eq!(g.players[0].library.len(),8);assert!(g.players[0].hand.is_empty());assert_eq!(g.players[0].graveyard.len(),4);assert!(!g.obj(ls[0]).tapped);assert!(g.obj(ls[2]).tapped);s.snapshot(None).unwrap();assert!(s.fault().is_none());}
}
#[test]
fn r608_2d_unimplemented_variable_or_other_player_choices_remain_refused(){
 for text in ["Untap up to X lands.","Target player untaps up to three lands."]{
  let d=CardDef::new("Boundary","{X}","Instant",text,None).unwrap();
  let reason=if text.starts_with("Untap") {"selected permanent action: fixed bounded count required"}
   else {"bounded tap selection: controller chooses a mandatory action"};
  crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
 }
}
