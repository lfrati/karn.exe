use crate::support::cite;
use game::{CardDef,Game,Move,Zone,Target,ObjId};
use game::state::{Answer,Choice,Decision};
use std::sync::OnceLock;
fn defs()->Vec<CardDef>{
 static DEFS:OnceLock<Vec<CardDef>>=OnceLock::new();
 DEFS.get_or_init(||{
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
 let def=CardDef::from_face(faces.iter().find(|f|f.face_name=="Ugin's Labyrinth").unwrap()).unwrap();game::play::check(&def).unwrap();
 vec![def,CardDef::new("Large","{7}","Artifact","",None).unwrap(),
 CardDef::new("Small","{6}","Artifact","",None).unwrap(),CardDef::new("Colored","{7}{R}","Creature — Dragon","",Some((5,5))).unwrap()]
 }).clone()
}
fn resolve(g:&mut Game){for _ in 0..20 {if g.stack.is_empty()||g.decision.is_some(){return;}g.act(g.priority,Move::Pass).unwrap();}panic!("stack");}
fn enter(g:&mut Game)->ObjId {let card=g.put(0,0,Zone::Hand);g.act(0,Move::PlayLand(card)).unwrap();let land=*g.battlefield.last().unwrap();resolve(g);land}
fn activate(g:&mut Game,land:ObjId,ability:usize){g.act(0,Move::Activate{source:land,ability,targets:vec![],paying:vec![]}).unwrap();}
#[test]
fn r607_2a_labyrinth_selects_matching_hand_card_then_mana_and_return_read_the_same_link(){
 let mut g=Game::new(defs(),3,20);let a=g.put(0,1,Zone::Hand);let b=g.put(0,1,Zone::Hand);g.put(0,2,Zone::Hand);g.put(0,3,Zone::Hand);
 let land=enter(&mut g);assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::May { possible: true }})));
 crate::support::refused(g.act(1,Move::Choose(Answer::Yes)), "that decision is another player's");cite!(["608.2d"]; refused("a decision is pending: a choice of the effect being applied (608.2d)"): g.act(0,Move::Pass));
 g.act(0,Move::Choose(Answer::Yes)).unwrap();
 assert!(matches!(&g.decision,Some(Decision::Choose{choice:Choice::Objects{verb:ir::Verb::Exile,among,count:1,..},..})if *among==vec![a,b]),"{:?}",g.decision);
 assert!(!g.sees(1,a));let mut clone=g.clone();
 for branch in [&mut g,&mut clone] {
 branch.act(0,Move::Choose(Answer::Objects(vec![a]))).unwrap();assert_eq!(branch.exile.len(),1);assert!(branch.sees(1,branch.exile[0]));
 activate(branch,land,1);cite!(["607.2a"]; assert_eq!(branch.players[0].pool.colorless,2,"linked exile doubles the mana immediately"));cite!(["605.3b"]; assert!(branch.stack.is_empty()));
 branch.set_tapped(land,false);activate(branch,land,2);resolve(branch);assert!(branch.exile.is_empty());assert_eq!(branch.players[0].hand.len(),4);
 branch.set_tapped(land,false);activate(branch,land,1);cite!(["607.2a"]; assert_eq!(branch.players[0].pool.colorless,3,"returned card no longer supports extra mana"));
 }
 assert_eq!(format!("{g:?}"),format!("{clone:?}"));
}
#[test]
fn r607_2a_another_labyrinth_and_a_new_incarnation_cannot_read_the_old_exile(){
 let mut g=Game::new(defs(),2,20);let card=g.put(0,1,Zone::Hand);let land=enter(&mut g);g.act(0,Move::Choose(Answer::Yes)).unwrap();
 g.act(0,Move::Choose(Answer::Objects(vec![card]))).unwrap();
 let other=g.put(0,0,Zone::Battlefield);activate(&mut g,other,1);cite!(["607.1"]; assert_eq!(g.players[0].pool.colorless,1));
 activate(&mut g,land,1);assert_eq!(g.players[0].pool.colorless,3);
 g.move_object(land,Zone::Graveyard);let old=g.players[0].graveyard[0];g.move_object(old,Zone::Battlefield);let new=*g.battlefield.last().unwrap();
 activate(&mut g,new,2);resolve(&mut g);cite!(["400.7","607.2a"]; assert_eq!(g.exile.len(),1,"same physical card is a new ability-link source"));
}
#[test]
fn r118_12_optional_hand_exile_decline_does_not_disclose_qualification(){
 for valid in [false,true]{let mut g=Game::new(defs(),2,20);g.put(0,if valid{1}else{2},Zone::Hand);let land=enter(&mut g);
 cite!(["402.3"]; assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::May { possible }} ) if possible == valid)));
 if !valid {cite!(["608.2d"]; refused("that option is impossible (608.2d)"): g.act(0,Move::Choose(Answer::Yes)));}
 g.act(0,Move::Choose(Answer::No)).unwrap();
 // Uncited (card text): "you may exile" was declined.
 assert!(g.exile.is_empty());activate(&mut g,land,1);cite!(["607.2a"]; case: format!("valid={valid}"); assert_eq!(g.players[0].pool.colorless,1));
 }
}
#[test]
fn r608_2h_leaving_before_the_trigger_resolves_still_exiles_for_that_old_source(){
 let mut g=Game::new(defs(),2,20);let large=g.put(0,1,Zone::Hand);let card=g.put(0,0,Zone::Hand);g.act(0,Move::PlayLand(card)).unwrap();let land=g.battlefield[0];
 assert_eq!(g.stack.len(),1);g.move_object(land,Zone::Graveyard);resolve(&mut g);g.act(0,Move::Choose(Answer::Yes)).unwrap();
 g.act(0,Move::Choose(Answer::Objects(vec![large]))).unwrap();cite!(["113.7a"]; assert_eq!(g.exile.len(),1));
 assert!(!g.is(Target::Object(land),ir::Sort::Permanent));
}
