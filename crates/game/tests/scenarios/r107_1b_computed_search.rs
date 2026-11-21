//! Arithmetic and cost-bound last known information compose in a search predicate.
use crate::support::{cite, paid};
use game::{CardDef,Game,Move,Zone,ObjId};
use game::state::{Decision,Choice,Answer};
fn game()->Game {
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
 let pod=CardDef::from_face(faces.iter().find(|f|f.name=="Birthing Pod").unwrap()).unwrap();
 game::play::check(&pod).unwrap();
 let bear=CardDef::new("Bear","{1}{G}","Creature — Bear","",Some((2,2))).unwrap();
 let wolf=CardDef::new("Wolf","{2}{G}","Creature — Wolf","",Some((3,3))).unwrap();
 Game::new(vec![pod,bear,wolf],2,20)
}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
fn activate(g:&mut Game,pod:ObjId,bear:ObjId,life:bool){
 g.players[0].pool.colors[4]=if life {1}else{2};
 g.act(0,paid(if life {"G"}else{"GG"},Move::WithLife(vec![life],Box::new(Move::Activate{source:pod,ability:0,targets:vec![],paying:vec![vec![bear]]})))).unwrap();
 pass(g);
}
#[test]
fn r608_2h_search_uses_the_sacrificed_objects_mana_value_after_the_cost_is_paid(){
 for life in [false,true] {
  let mut g=game();let pod=g.put(0,0,Zone::Battlefield);let bear=g.put(0,1,Zone::Battlefield);
  let wrong=g.put(0,1,Zone::Library);let right=g.put(0,2,Zone::Library);
  activate(&mut g,pod,bear,life);
  assert!(!g.battlefield.contains(&bear));assert!(g.obj(pod).tapped);cite!(["107.4f"]; assert_eq!(g.players[0].life,if life {18}else{20}));
  let Some(Decision::Choose{player:0,choice:Choice::Search{among,..}})=&g.decision else {panic!("{:?}",g.decision)};
  cite!(["608.2h"]; assert_eq!(*among,vec![right]));
  cite!(["701.23a"]; refused("a chosen card is not one the search can find, in the searched zone and matching its description (701.23a)"): g.act(0,Move::Choose(Answer::Objects(vec![wrong]))));
  let clone=g.clone();for mut branch in [g,clone] {
   branch.act(0,Move::Choose(Answer::Objects(vec![right]))).unwrap();
   assert_eq!(branch.battlefield.iter().filter(|o|branch.obj(**o).card==Some(2)).count(),1);
   assert_eq!(branch.players[0].library,vec![wrong]);assert!(branch.decision.is_none());
  }
 }
}
#[test]
fn r701_23b_the_qualified_search_can_fail_without_refunding_its_cost(){
 let mut g=game();let pod=g.put(0,0,Zone::Battlefield);let bear=g.put(0,1,Zone::Battlefield);
 g.put(0,2,Zone::Library);activate(&mut g,pod,bear,true);
 cite!(["701.23b"]; ok: g.act(0,Move::Choose(Answer::Objects(vec![]))));
 cite!(["701.23b"]; assert_eq!(g.players[0].library.len(),1));assert_eq!(g.players[0].life,18);
 assert_eq!(g.battlefield,vec![pod]);assert!(g.obj(pod).tapped);
}
