//! Full admitted cast-trigger cards: announcement facts, source continuity and later triggers.
use crate::support::cite;
use game::{CardDef,Game,Move,Target,Zone,ObjId};
use game::state::{Answer,Choice,Decision};
use std::sync::OnceLock;
fn defs()->Vec<CardDef>{static DEFS:OnceLock<Vec<CardDef>>=OnceLock::new();DEFS.get_or_init(||{
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
 let mut defs:Vec<_>=["Sowing Mycospawn","Thief of Existence"].into_iter().map(|n|CardDef::from_face(faces.iter().find(|f|f.face_name==n).unwrap()).unwrap()).collect();
 for (n,k,t) in [("Land","Land",""),("Artifact","Artifact",""),("Counter","Instant","Counter target spell."),("Destroy","Instant","Destroy target creature.")]{defs.push(CardDef::new(n,"{0}",k,t,None).unwrap());}
 for d in &defs {game::play::check(d).unwrap();}defs
}).clone()}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
fn cast(g:&mut Game,def:usize,targets:Vec<Vec<Target>>){let card=g.put(0,def,Zone::Hand);g.act(0,crate::support::paid(["CCCG", "CCG", "", "", "", ""][def], Move::Cast{card,targets})).unwrap();}
fn resolve(g:&mut Game,target:Target,found:Option<ObjId>){for _ in 0..40{
 match g.decision.clone(){
 Some(Decision::Order{player,abilities})=>g.act(player,Move::Order((0..abilities.len()).collect())).unwrap(),
 Some(Decision::Targets{player,..})=>g.act(player,Move::ChooseTargets(vec![vec![target]])).unwrap(),
 Some(Decision::Choose{player,choice:Choice::Search{among,..}})=>{let cards=found.into_iter().collect::<Vec<_>>();assert!(cards.iter().all(|c|among.contains(c)));g.act(player,Move::Choose(Answer::Objects(cards))).unwrap();},
 Some(other)=>panic!("unexpected {other:?}"),None if g.stack.is_empty()=>return,None=>{g.act(g.priority,Move::Pass).unwrap();}
 }
}panic!("stack");}
#[test]
fn r603_2_sowing_kicker_search_and_exile_are_cast_triggers_independent_of_the_spell(){
 for kicked in [false,true]{let mut g=Game::new(defs(),3,20);let enemy=g.put(2,2,Zone::Battlefield);let found=g.put(0,2,Zone::Library);let card=g.put(0,0,Zone::Hand);
 g.players[0].pool.colors[4]=1;g.players[0].pool.colorless=if kicked{5}else{3};
 g.act(0,crate::support::paid(if kicked { "CCCCCG" } else { "CCCG" }, Move::WithKicks(u32::from(kicked),Box::new(Move::Cast{card,targets:vec![]})))).unwrap();
 // Place all cast triggers, then counter the creature before either trigger resolves.
 if let Some(Decision::Order{abilities,..})=&g.decision{let n=abilities.len();assert_eq!(n,2);g.act(0,Move::Order((0..n).collect())).unwrap();}
 if kicked {g.act(0,Move::ChooseTargets(vec![vec![Target::Object(enemy)]])).unwrap();}
 let spell=g.stack.iter().copied().find(|o|g.obj(*o).ability.is_none()).unwrap();
 cast(&mut g,4,vec![vec![Target::Object(spell)]]);pass(&mut g);
 let mut copy=g.clone();for branch in [&mut g,&mut copy]{resolve(branch,Target::Object(enemy),Some(found));
 cite!(["113.7a"];assert!(branch.battlefield.iter().any(|o|branch.obj(*o).chars.name=="Land"&&branch.controller(*o)==0)));
 cite!(["603.4"];assert_eq!(branch.exile.len(),usize::from(kicked)));cite!(["701.6a"];assert!(branch.players[0].graveyard.iter().any(|o|branch.obj(*o).chars.name=="Sowing Mycospawn")));
 }
 assert_eq!(format!("{g:?}"),format!("{copy:?}"));
 }
}
#[test]
fn r400_7a_thief_grant_follows_spell_to_permanent_and_chooses_any_opponent_on_departure(){
 let mut g=Game::new(defs(),3,20);let victim=g.put(2,3,Zone::Battlefield);g.put(1,3,Zone::Library);g.put(2,3,Zone::Library);
 g.players[0].pool.colorless=2;g.players[0].pool.colors[4]=1;cast(&mut g,1,vec![]);
 g.act(0,Move::ChooseTargets(vec![vec![Target::Object(victim)]])).unwrap();resolve(&mut g,Target::Object(victim),None);
 assert_eq!(g.exile.len(),1);let thief=g.battlefield[0];assert_eq!(g.obj(thief).chars.name,"Thief of Existence");
 cast(&mut g,5,vec![vec![Target::Object(thief)]]);pass(&mut g);
 assert!(matches!(g.decision,Some(Decision::Targets{player:0,..})));
 resolve(&mut g,Target::Player(1),None);cite!(["400.7a"];assert_eq!(g.players[1].hand.len(),1));assert!(g.players[2].hand.is_empty());
 // No return clause exists: the exiled permanent stays exiled.
 assert_eq!(g.exile.len(),1);
}
#[test]
fn r118_12_declining_thiefs_optional_target_does_not_grant_a_departure_trigger(){
 let mut g=Game::new(defs(),2,20);g.players[0].pool.colorless=2;g.players[0].pool.colors[4]=1;cast(&mut g,1,vec![]);
 g.act(0,Move::ChooseTargets(vec![vec![]])).unwrap();resolve(&mut g,Target::Player(1),None);
 let thief=g.battlefield[0];cast(&mut g,5,vec![vec![Target::Object(thief)]]);resolve(&mut g,Target::Player(1),None);
 // no target chosen for "up to one", so nothing is exiled and "If you do" finds the exile not done: no grant, no draw
 cite!(["118.12"];assert!(g.players[1].hand.is_empty()));cite!(["603.3d","601.2c"];assert!(g.exile.is_empty()));
}
