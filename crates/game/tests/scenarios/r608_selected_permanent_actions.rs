//! Resolution selection is not targeting or cost payment (608.2d/609.3/701.26).
use crate::support::cite;
use game::{CardDef,Game,Move,ObjId,Target,Zone};
use game::state::{Answer,Choice,Decision};
fn card(name:&str,ty:&str,text:&str,pt:Option<(i32,i32)>)->CardDef{CardDef::new(name,"{0}",ty,text,pt).unwrap()}
fn game(text:&str)->Game{
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
 let pinned=|name:&str|CardDef::from_faces(CardDef::catalog(&faces).get(name).unwrap()).unwrap();
 let ds=vec![card("Effect","Sorcery",text,None),pinned("Selesnya Sanctuary"),pinned("Salvage Scuttler"),
 card("Land","Land","",None),card("Artifact","Artifact","Shroud",None),
 card("Steal","Instant","Gain control of target permanent until end of turn.",None),
 card("Leader","Legendary Artifact Creature — Human","",Some((2,2)))];
 for d in &ds{game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}Game::new(ds,3,20)
}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
fn cast(g:&mut Game,index:usize,targets:Vec<Vec<Target>>){let card=g.put(0,index,Zone::Hand);g.act(0,Move::Cast{card,targets}).unwrap();pass(g);}
fn choose(g:&mut Game,p:usize,objects:Vec<ObjId>){g.act(p,Move::Choose(Answer::Objects(objects))).unwrap();}
#[test]
fn full_sanctuary_alone_returns_itself_and_other_lands_are_chosen_at_resolution(){
 for other in [false,true]{let mut g=game("You gain 1 life.");let land=other.then(||g.put(0,3,Zone::Battlefield));
  let hand=g.put(0,1,Zone::Hand);g.act(0,Move::PlayLand(hand)).unwrap();let sanctuary=*g.battlefield.last().unwrap();
  assert!(g.obj(sanctuary).tapped);pass(&mut g);
  if let Some(land)=land {cite!(["608.2d"]; assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Objects{count:1,..}}))));
   crate::support::refused(g.act(1,Move::Choose(Answer::Objects(vec![land]))), "that decision is another player's");choose(&mut g,0,vec![land]);
   cite!(["107.5"]; refused("Selesnya Sanctuary can't pay {T} as it is (107.5, 118.3)"): g.act(0,Move::Activate{source:sanctuary,ability:2,targets:vec![],paying:vec![]})); // enters tapped
   g.set_tapped(sanctuary,false);g.act(0,Move::Activate{source:sanctuary,ability:2,targets:vec![],paying:vec![]}).unwrap();
   assert_eq!(g.players[0].pool.colors[0],1);assert_eq!(g.players[0].pool.colors[4],1);
  }else{assert!(g.battlefield.is_empty());}
  assert_eq!(g.players[0].hand.len(),1);
 }
}
#[test]
fn full_scuttler_attack_returns_nontargetable_artifact_or_nothing(){
 for count in [0,1,2]{let mut g=game("You gain 1 life.");let scuttler=g.put(0,2,Zone::Battlefield);
  let artifacts:Vec<_>=(0..count).map(|_|g.put(0,4,Zone::Battlefield)).collect();pass(&mut g);pass(&mut g);
  g.act(0,Move::Attack(vec![(scuttler,Target::Player(1))])).unwrap();pass(&mut g);
  if count==2{choose(&mut g,0,vec![artifacts[1]]);}cite!(["115.10a"]; case: count; assert_eq!(g.players[0].hand.len(),usize::from(count>0)));
  assert!(g.battlefield.contains(&scuttler));
 }
}
#[test]
fn short_exact_returns_and_tokens_use_effect_rules_not_cost_or_card_rules(){
 for count in [0,1,2]{let mut g=game("Return three artifacts you control to their owners' hands.");
  for _ in 0..count{g.put(0,4,Zone::Battlefield);}cast(&mut g,0,vec![]);
  cite!(["609.3"]; case: count; assert_eq!(g.players[0].hand.len(),count));assert!(g.decision.is_none());
 }
 let mut g=game("Return an artifact you control to its owner's hand.");let model=g.put(1,4,Zone::Battlefield);
 let token=g.create_token(0,g.obj(model).chars.clone(),false);cast(&mut g,0,vec![]);
 assert!(!g.battlefield.contains(&token));cite!(["111.7"]; assert!(g.players[0].hand.is_empty()));
}
#[test]
fn targeted_actor_chooses_their_objects_but_you_still_means_the_effect_controller(){
 for ours in [false,true]{let text=if ours{"Target player returns a land you control to its owner's hand."}else{"Target player returns a land they control to its owner's hand."};
  let mut g=game(text);let a=g.put(0,3,Zone::Battlefield);let b=g.put(0,3,Zone::Battlefield);
  let c=g.put(1,3,Zone::Battlefield);let d=g.put(1,3,Zone::Battlefield);cast(&mut g,0,vec![vec![Target::Player(1)]]);
  let Some(Decision::Choose{player:1,choice:Choice::Objects{among,..}})=&g.decision else{panic!("{:?}",g.decision)};
  cite!(["109.5"]; case: ours; assert_eq!(*among,if ours{vec![a,b]}else{vec![c,d]}));let chosen=if ours{a}else{c};
  crate::support::refused(g.act(0,Move::Choose(Answer::Objects(vec![chosen]))), "that decision is another player's");let mut copy=g.clone();
  for g in [&mut g,&mut copy]{choose(g,1,vec![chosen]);assert_eq!(g.players[usize::from(!ours)].hand.len(),1);}
 }
}
#[test]
fn stolen_commander_returns_to_owner_and_destination_choice_clones_do_not_reselect(){
 let mut g=game("Return an artifact you control to its owner's hand.");
 let leader=g.put(1,6,Zone::Command);g.designate_commander(leader).unwrap();g.move_object(leader,Zone::Battlefield);
 let leader=*g.battlefield.last().unwrap();cast(&mut g,5,vec![vec![Target::Object(leader)]]);
 let other=g.put(0,4,Zone::Battlefield);cast(&mut g,0,vec![]);choose(&mut g,0,vec![leader]);
 cite!(["903.9b"]; assert!(matches!(g.decision,Some(Decision::Choose{player:1,choice:Choice::CommandZone{..}}))));
 let mut copy=g.clone();g.act(1,Move::Choose(Answer::Yes)).unwrap();copy.act(1,Move::Choose(Answer::No)).unwrap();
 cite!(["903.9b"]; assert_eq!(g.players[1].hand.len(),0));cite!(["903.9b"]; assert_eq!(copy.players[1].hand.len(),1));
 for g in [&g,&copy]{assert!(g.battlefield.contains(&other));assert!(g.decision.is_none());}
}
#[test]
fn tap_choices_require_legal_status_but_named_untap_attempts_still_do_as_much_as_possible(){
 for tap in [false,true]{let mut g=game(if tap{"Tap a land."}else{"Untap a land."});
  let a=g.put(0,3,Zone::Battlefield);let b=g.put(1,3,Zone::Battlefield);let impossible=g.put(2,3,Zone::Battlefield);
  g.set_tapped(a,!tap);g.set_tapped(b,!tap);g.set_tapped(impossible,tap);if !tap{g.put_counters(a,"stun",1);}
  cast(&mut g,0,vec![]);let Some(Decision::Choose{choice:Choice::Objects{among,..},..})=&g.decision else{panic!()};
  cite!(["608.2d"]; case: tap; assert_eq!(among,&vec![a,b]));let why=format!("a chosen object is not among the offered objects to {} (608.2d)",if tap{"tap"}else{"untap"});cite!(["608.2d"]; case: tap; refused(&why): g.act(0,Move::Choose(Answer::Objects(vec![impossible]))));choose(&mut g,0,vec![a]);
  assert_eq!(g.obj(a).tapped,true);if !tap{cite!(["122.1d"]; assert!(!g.obj(a).counters.contains_key("stun")));}
 }
 let mut g=game("Untap target land. You gain 1 life.");let land=g.put(0,3,Zone::Battlefield);
 cast(&mut g,0,vec![vec![Target::Object(land)]]);assert_eq!(g.players[0].life,21);assert!(!g.obj(land).tapped);
}
#[test]
fn unsupported_chooser_counts_optional_and_other_operations_are_refused(){
 for text in ["You may return two lands you control to their owners' hands.","Return X lands you control to their owners' hands.",
 "Return a land you control and an artifact you control to their owners' hands.",
 "Each player returns a land they control to its owner's hand."]{
  let d=CardDef::new("Refused","{0}","Sorcery",text,None).unwrap();
  let reason=if text.starts_with("Return X") {"selected permanent action: fixed bounded count required"}
   else {"selected return: one mandatory actor and object group, no per-player iteration"};
  crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
 }
}
