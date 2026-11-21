use crate::support::{cite,paid};
use game::{CardDef,Game,Move,Zone,Target,ObjId};
use game::state::{Answer,Choice,Decision};
use std::sync::OnceLock;
fn boseiju()->CardDef {
 static DEF:OnceLock<CardDef>=OnceLock::new();
 DEF.get_or_init(||{let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
 let d=CardDef::from_face(faces.iter().find(|f|f.face_name=="Boseiju, Who Endures").unwrap()).unwrap();
 game::play::check(&d).unwrap();d}).clone()
}
fn setup()->(Game,ObjId,ObjId) {
 let mut g=Game::new(vec![boseiju(),
 CardDef::new("Artifact","{0}","Artifact","",None).unwrap(),
 CardDef::new("Dual","","Land — Forest Island","",None).unwrap(),
 CardDef::new("Wastes","","Basic Land","",None).unwrap(),
 CardDef::new("Legend","{0}","Legendary Creature — Elf","",Some((1,1))).unwrap(),
 CardDef::new("Other land","","Land","",None).unwrap(),
 ],3,20);
 let source=g.put(0,0,Zone::Hand);let target=g.put(2,1,Zone::Battlefield);(g,source,target)
}
fn channel(source:ObjId,target:ObjId,units:&str)->Move {paid(units,Move::Activate{source,ability:1,targets:vec![vec![Target::Object(target)]],paying:vec![]})}
fn resolve(g:&mut Game){for _ in 0..20 {if g.stack.is_empty()||g.decision.is_some(){return;}g.act(g.priority,Move::Pass).unwrap();}panic!("stack");}
#[test]
fn r113_6m_channel_is_only_owned_in_hand_and_uses_the_shared_activation_payment(){
 let (mut g,source,target)=setup();g.players[0].pool.colors[4]=1;
 assert!(!g.clone().act(0, channel(source,target,"CG")).is_ok());assert!(!g.clone().act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).is_ok());
 let before=format!("{g:?}");cite!(["118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0,channel(source,target,"G")));assert_eq!(format!("{g:?}"),before);
 g.players[0].pool.colorless=1;cite!(["113.6m"]; assert!(g.clone().act(0, channel(source,target,"CG")).is_ok()));
 g.act(0,channel(source,target,"CG")).unwrap();cite!(["601.2h"]; assert_eq!(g.players[0].pool.total(),0));assert!(g.players[0].hand.is_empty());cite!(["601.2h"]; assert_eq!(g.players[0].graveyard.len(),1));
 let gy=g.players[0].graveyard[0];cite!(["113.6m"]; assert!(!g.clone().act(0, channel(gy,target,"CG")).is_ok()));
 let (mut g,_,target)=setup();let on=g.put(0,0,Zone::Battlefield);g.players[0].pool.colors[4]=2;
 assert!(!g.clone().act(0, channel(on,target,"CG")).is_ok());cite!(["113.6m"]; refused("that ability functions only in the zone it moves its card out of (113.6m)"): g.act(0,channel(on,target,"CG")));
 g.act(0,Move::Activate{source:on,ability:0,targets:vec![],paying:vec![]}).unwrap();assert_eq!(g.players[0].pool.colors[4],3);
}
#[test]
fn r602_2b_self_activation_reduction_counts_own_legendary_creatures_and_preserves_green(){
 for legends in 0..=2 {
 let (mut g,source,target)=setup();for _ in 0..legends{g.put(0,4,Zone::Battlefield);}g.put(1,4,Zone::Battlefield);
 g.players[0].pool.colorless=3;assert!(!g.clone().act(0, channel(source,target,if legends==0{"CG"}else{"G"})).is_ok());
 g.players[0].pool.colorless=u32::from(legends==0);g.players[0].pool.colors[4]=1;
 cite!(["602.2b", "601.2f"]; case: format!("legends={legends}"); assert!(g.clone().act(0, channel(source,target,if legends==0{"CG"}else{"G"})).is_ok()));g.act(0,channel(source,target,if legends==0{"CG"}else{"G"})).unwrap();assert_eq!(g.players[0].pool.total(),0);
 }
}
#[test]
fn r608_2h_named_controller_owns_optional_search_after_destruction_with_clone_and_private_view(){
 let (mut g,source,target)=setup();g.players[0].pool.colors[4]=1;g.players[0].pool.colorless=1;
 let dual=g.put(2,2,Zone::Library);let wastes=g.put(2,3,Zone::Library);let other=g.put(2,5,Zone::Library);
 g.act(0,channel(source,target,"CG")).unwrap();resolve(&mut g);
 assert_eq!(g.players[2].graveyard.len(),1);cite!(["608.2h"]; assert!(matches!(g.decision,Some(Decision::Choose{player:2,choice:Choice::May}))));
 crate::support::refused(g.act(0,Move::Choose(Answer::Yes)), "that decision is another player's");crate::support::refused(g.act(2,Move::Pass), "a decision is pending: a choice of the effect being applied (608.2d)");
 let mut no=g.clone();no.act(2,Move::Choose(Answer::No)).unwrap();assert_eq!(no.players[2].library.len(),3);
 g.act(2,Move::Choose(Answer::Yes)).unwrap();
 cite!(["701.23a"]; assert!(matches!(&g.decision,Some(Decision::Choose{player:2,choice:Choice::Search{among,..}})if *among==vec![dual])));
 for c in [dual,wastes,other]{cite!(["701.23a"]; case: format!("{c:?}"); assert!(g.sees(2,c)));cite!(["401.2"]; case: format!("{c:?}"); assert!(!g.sees(0,c)));cite!(["401.2"]; case: format!("{c:?}"); assert!(!g.sees(1,c)));}
 cite!(["701.23a"]; refused("a chosen card is not one the search can find, in the searched zone and matching its description (701.23a): choose between 0 and 1 distinct matching objects"): g.act(2,Move::Choose(Answer::Objects(vec![wastes]))));
 let mut copy=g.clone();for branch in [&mut g,&mut copy] {branch.act(2,Move::Choose(Answer::Objects(vec![dual]))).unwrap();assert_eq!(branch.players[2].library.len(),2);cite!(["701.23a"]; assert!(branch.battlefield.iter().any(|o|branch.obj(*o).chars.name=="Dual"&&branch.controller(*o)==2)));}
 assert_eq!(format!("{g:?}"),format!("{copy:?}"));
}
#[test]
fn r608_2b_illegal_target_prevents_the_following_search(){
 let (mut g,source,target)=setup();g.players[0].pool.colors[4]=1;g.players[0].pool.colorless=1;g.put(2,2,Zone::Library);
 g.act(0,channel(source,target,"CG")).unwrap();g.move_object(target,Zone::Graveyard);resolve(&mut g);cite!(["608.2b"]; assert!(g.decision.is_none()));cite!(["608.2b"]; assert_eq!(g.players[2].library.len(),1));
}

#[test]
fn r603_4_controller_named_in_intervening_trigger_is_available_to_following_instructions(){
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
 let vraska=CardDef::from_face(faces.iter().find(|f|f.face_name=="Vraska, the Cutting Glare").unwrap()).unwrap();game::play::check(&vraska).unwrap();
 let mut g=Game::new(vec![vraska,CardDef::new("Land","","Land","",None).unwrap(),CardDef::new("Victim","{0}","Artifact","",None).unwrap()],3,20);
 for _ in 0..6{g.put(0,1,Zone::Battlefield);}let victim=g.put(2,2,Zone::Battlefield);
 let v=g.put(0,0,Zone::Hand);g.players[0].pool.colorless=20;g.players[0].pool.colors=[20;5];
 g.act(0,paid("BBG",Move::Cast{card:v,targets:vec![]})).unwrap();resolve(&mut g);
 g.act(0,Move::ChooseTargets(vec![vec![Target::Object(victim)]])).unwrap();resolve(&mut g);
 cite!(["603.4"]; assert_eq!(g.players[2].graveyard.len(),1));
 cite!(["608.2h"]; assert!(g.battlefield.iter().any(|o|g.obj(*o).chars.subtypes.iter().any(|s|s.name()=="Treasure")&&g.controller(*o)==2)));
}
