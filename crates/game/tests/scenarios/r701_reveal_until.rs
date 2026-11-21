//! Reveal-until produces both the revealed prefix and its distinguished stopping card.
use game::{CardDef, Game, Move, Zone};
use game::binding::{Kind, Value};
use game::state::{Answer,Choice,Decision};
use game::simulation::{ResponseError,Simulation};
use crate::support::{cite, refused};
use game::observation::Kind as Observation;
use std::sync::OnceLock;
fn defs()->Vec<CardDef>{
 static DEFS:OnceLock<Vec<CardDef>>=OnceLock::new();DEFS.get_or_init(||{
  let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
  let d=CardDef::from_faces(CardDef::catalog(&faces).get("Demonic Consultation").unwrap()).unwrap();game::play::check(&d).unwrap();
  let land=|n:&str|CardDef::new(n,"",&format!("Basic Land — {n}"),"",None).unwrap();
  vec![d,land("Island"),land("Forest"),land("Plains")]
 }).clone()
}
fn start(top:&[usize])->Game{
 let mut g=Game::new(defs(),3,20);for &c in top.iter().rev(){g.put(0,c,Zone::Library);}
 let card=g.put(0,0,Zone::Hand);g.players[0].pool.colors[2]=1;
 g.act(0,crate::support::paid("B", Move::Cast{card,targets:vec![]})).unwrap();for _ in 0..3{g.act(g.priority,Move::Pass).unwrap();}
 assert_eq!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Value(Kind::CardName)}));g
}
fn choose(g:&mut Game,name:&str){g.act(0,Move::Choose(Answer::Value(Value::CardName(name.into())))).unwrap();}
#[test]
fn r608_2c_consultation_exiles_six_then_reveals_only_through_the_named_card(){
 let mut g=start(&[1,1,1,1,1,1,3,2,2,3]);choose(&mut g,"Forest");
 cite!(["608.2c"]; assert_eq!(g.exile.len(),7));cite!(["608.2c"]; assert_eq!(g.players[0].hand.len(),1));
 cite!(["608.2c"]; assert_eq!(g.obj(g.players[0].hand[0]).card,Some(2)));
 assert_eq!(g.players[0].library.iter().map(|c|g.obj(*c).card.unwrap()).collect::<Vec<_>>(),vec![3,2]);
 assert!(g.stack.is_empty());
 for p in 0..3 {let seen=g.observations(p);assert_eq!(seen.len(),1);cite!(["701.20a"]; case: p; assert_eq!(seen[0].kind,Observation::Revealed));assert_eq!(seen[0].cards.iter().map(|c|c.name.as_str()).collect::<Vec<_>>(),["Plains","Forest"]);}
}
#[test]
fn r609_3_empty_short_and_absent_named_cards_do_not_invent_a_match_or_draw(){
 for top in [vec![],vec![2],vec![2;6],vec![2,1,1,1,1,1,3,1],vec![1;8]]{
  let mut g=start(&top);choose(&mut g,"Forest");cite!(["609.3"]; case: format!("{top:?}"); assert_eq!(g.exile.len(),top.len()));
  cite!(["609.3"]; case: format!("{top:?}"); assert!(g.players[0].hand.is_empty() && g.players[0].library.is_empty()));assert!(!g.players[0].lost);
 }
}
#[test]
fn r608_2d_named_choice_precedes_exile_and_managed_clones_retain_distinct_answers(){
 let g=start(&[1,1,1,1,1,1,3,2,1]);assert_eq!(g.players[0].library.len(),9);cite!(["608.2c", "608.2d"]; assert!(g.exile.is_empty()));
 let mut sim=Simulation::new(g);let req=sim.request().unwrap().unwrap().clone();
 assert_eq!(sim.respond(req.id,1,Move::Choose(Answer::Value(Value::CardName("Forest".into())))),Err(ResponseError::WrongPlayer{expected:0,received:1}));
 refused(sim.respond(req.id,0,Move::Pass),"a decision is pending: a choice of the effect being applied (608.2d)");
 refused(sim.respond(req.id,0,Move::Choose(Answer::Value(Value::CardName("not a card".into())))),"choose the name of a card in the Oracle card reference (201.4)");
 let mut copy=sim.clone();
 for (s,name,exiled,remaining) in [(&mut sim,"Forest",7,1),(&mut copy,"Plains",6,2)]{
  s.respond(req.id,0,Move::Choose(Answer::Value(Value::CardName(name.into())))).unwrap();s.request().unwrap();
  let g=s.game().unwrap();cite!(["608.2c"]; case: name; assert_eq!(g.exile.len(),exiled));assert_eq!(g.players[0].library.len(),remaining);assert_eq!(g.players[0].hand.len(),1);
  for p in 0..3{assert_eq!(s.take_observations(p).unwrap().len(),1);assert!(s.take_observations(p).unwrap().is_empty());}
  assert!(matches!(s.respond(req.id,0,Move::Pass),Err(ResponseError::Stale{received,..}) if received==req.id));
 }
}
