//! Magecraft is an ordinary alternative trigger pattern over cast and spell-copy facts.
use game::{CardDef, Game, Move, ObjId, Target, Zone, Event};
use game::state::{Answer, Choice, Decision};
use std::sync::OnceLock;
use crate::support::{cite, paid};
fn card(name:&str,types:&str,text:&str,pt:Option<(i32,i32)>)->CardDef{CardDef::new(name,"{0}",types,text,pt).unwrap()}
fn archmage()->CardDef{
 static CARD:OnceLock<CardDef>=OnceLock::new();CARD.get_or_init(||{
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
 let d=CardDef::from_face(faces.iter().find(|f|f.name=="Archmage Emeritus").unwrap()).unwrap();game::play::check(&d).unwrap();d
 }).clone()
}
fn setup()->Game{
 let ds=vec![archmage(),card("Brief","Instant","You gain 1 life.",None),card("Long","Sorcery","You gain 1 life.",None),
 card("Bear","Creature — Bear","",Some((2,2))),card("Echo","Instant","Replicate {1}\nYou gain 2 life.",None),
 card("Fork","Instant","Copy target spell. You may choose new targets for the copy.",None),
 card("Duplicate","Instant","Create a token that's a copy of target creature.",None),
 card("Spark","Instant","Replicate {1}\nSpark deals 1 damage to any target.",None)];
 for d in &ds{game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}
 let mut g=Game::new(ds,2,20);for p in 0..2{for _ in 0..15{g.put(p,3,Zone::Library);}}g
}
fn pass(g:&mut Game){g.act(g.priority,Move::Pass).unwrap();g.act(g.priority,Move::Pass).unwrap();}
fn order(g:&mut Game){if let Some(Decision::Order{player,abilities})=&g.decision{g.act(*player,Move::Order((0..abilities.len()).collect())).unwrap();}}
fn drain(g:&mut Game){for _ in 0..30{order(g);if g.stack.is_empty(){return;}pass(g);}panic!("stack did not drain");}
fn cast(g:&mut Game,p:usize,index:usize,targets:Vec<Vec<Target>>,spending:&str)->ObjId{let c=g.put(p,index,Zone::Hand);g.act(p,paid(spending,Move::Cast{card:c,targets})).unwrap();c}
#[test]
fn r603_full_archmage_oracle_filters_both_event_verbs_to_its_controllers_instant_and_sorcery_spells(){
 let mut g=setup();g.players[0].pool.colors[1]=4;cast(&mut g,0,0,vec![], "UUUU");assert_eq!(g.stack.len(),1);pass(&mut g);
 assert!(g.players[0].hand.is_empty());
 for index in [1,2]{cast(&mut g,0,index,vec![], "");cite!(["603.2"]; case: format!("{index}"); assert_eq!(g.stack.len(),2));drain(&mut g);}
 assert_eq!(g.players[0].hand.len(),2);cast(&mut g,0,3,vec![], "");cite!(["603.2"]; assert_eq!(g.stack.len(),1));drain(&mut g);assert_eq!(g.players[0].hand.len(),2);
 g.act(0,Move::Pass).unwrap();cast(&mut g,1,1,vec![], "");assert_eq!(g.stack.len(),1);drain(&mut g);assert_eq!(g.players[0].hand.len(),2);
}
#[test]
fn r603_replicate_magecraft_triggers_once_for_cast_and_once_per_spell_copy(){
 let mut g=setup();g.put(0,0,Zone::Battlefield);g.players[0].pool.colorless=3;let c=g.put(0,4,Zone::Hand);
 g.act(0,paid("CCC",Move::WithReplicate(3,Box::new(Move::Cast{card:c,targets:vec![]})))).unwrap();drain(&mut g);
 cite!(["603.2"]; assert_eq!(g.players[0].hand.len(),4));cite!(["702.56a"]; assert_eq!(g.players[0].life,28));
 cite!(["707.10"]; assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Cast{..})).count(),1));cite!(["702.56a"]; assert_eq!(g.log.iter().filter(|e|matches!(e,Event::SpellCopied{..})).count(),3));
}
#[test]
fn r603_copying_an_opponents_spell_uses_the_copying_controller_for_magecraft(){
 let mut g=setup();g.put(0,0,Zone::Battlefield);g.put(1,0,Zone::Battlefield);cast(&mut g,0,1,vec![], "");
 pass(&mut g);assert_eq!(g.players[0].hand.len(),1);let original=g.stack[0];
 g.act(0,Move::Pass).unwrap();cast(&mut g,1,5,vec![vec![Target::Object(original)]], "");drain(&mut g);
 assert_eq!(g.players[0].hand.len(),1);cite!(["707.10"]; assert_eq!(g.players[1].hand.len(),2));assert_eq!(g.players[0].life,21);assert_eq!(g.players[1].life,21);
 assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Cast{..})).count(),2);cite!(["707.10"]; assert_eq!(g.log.iter().filter(|e|matches!(e,Event::SpellCopied{by:1,..})).count(),1));
}
#[test]
fn r603_token_copies_and_permanent_spell_copies_do_not_match_magecraft(){
 let mut g=setup();let mage=g.put(0,0,Zone::Battlefield);cast(&mut g,0,6,vec![vec![Target::Object(mage)]], "");drain(&mut g);
 assert_eq!(g.players[0].hand.len(),1);assert_eq!(g.battlefield.len(),2);cite!(["707.10"]; assert!(!g.log.iter().any(|e|matches!(e,Event::SpellCopied{..}))));
 let permanent=g.put(1,3,Zone::Stack);cast(&mut g,0,5,vec![vec![Target::Object(permanent)]], "");drain(&mut g);
 // Uncited (card text): "Whenever you cast or copy an instant or sorcery spell": a creature spell copy does not match (the second Fork occurrence is the token copy's own magecraft).
 assert_eq!(g.players[0].hand.len(),3,"two magecraft occurrences for casting Fork; none for copying a creature spell");
 cite!(["707.10"]; assert_eq!(g.log.iter().filter(|e|matches!(e,Event::SpellCopied{..})).count(),1));
}
#[test]
fn r603_copy_trigger_survives_cloned_suspended_target_selection_without_repeating_cast(){
 use game::simulation::Simulation;
 let mut g=setup();g.put(0,0,Zone::Battlefield);g.players[0].pool.colorless=1;let c=g.put(0,7,Zone::Hand);
 g.act(0,paid("C",Move::WithReplicate(1,Box::new(Move::Cast{card:c,targets:vec![vec![Target::Player(1)]]})))).unwrap();
 for _ in 0..6{order(&mut g);if matches!(g.decision,Some(Decision::Choose{choice:Choice::Retarget{..},..})){break;}pass(&mut g);}
 assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::Retarget{..},..})));
 let mut sim=Simulation::new(g);let r=sim.request().unwrap().unwrap().clone();sim.snapshot(None).unwrap();let mut cloned=sim.clone();
 for s in [&mut sim,&mut cloned]{s.respond(r.id,r.player,Move::Choose(Answer::No)).unwrap();
  for _ in 0..20{if s.game().unwrap().stack.is_empty(){break;}let r=s.request().unwrap().unwrap().clone();s.respond(r.id,r.player,Move::Pass).unwrap();}
  assert!(s.game().unwrap().stack.is_empty());cite!(["707.10"]; assert_eq!(s.game().unwrap().players[0].hand.len(),2));assert_eq!(s.game().unwrap().players[1].life,18);s.snapshot(Some(0)).unwrap();assert!(s.fault().is_none());
 }
}
