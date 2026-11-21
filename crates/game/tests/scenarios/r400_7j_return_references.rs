//! Returned object actions and amounts read from the original card are distinct (400.7j/608.2h).
use crate::support::{cite, paid};
use game::{CardDef,Game,Move,Target,Zone};
use game::state::Answer;
fn pinned(name:&str)->CardDef{
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
 let d=CardDef::from_faces(CardDef::catalog(&faces).get(name).unwrap()).unwrap();game::play::check(&d).unwrap();d
}
fn big()->CardDef{CardDef::new("Big","{5}{G}","Creature — Beast","",Some((6,6))).unwrap()}
fn pass(g:&mut Game){for _ in 0..3{g.act(g.priority,Move::Pass).unwrap();}}
fn cast(g:&mut Game,def:usize,target:game::ObjId,spending:&str){g.players[0].pool.colors=[10;5];let c=g.put(0,def,Zone::Hand);g.act(0,paid(spending,Move::Cast{card:c,targets:vec![vec![Target::Object(target)]]})).unwrap();pass(g);}
#[test]
fn reanimate_reads_the_original_clone_card_after_copy_entry_and_clone_suspension(){
 let mut g=Game::new(vec![pinned("Reanimate"),pinned("Clone"),big()],3,20);
 let big=g.put(1,2,Zone::Battlefield);let dead=g.put(0,1,Zone::Graveyard);cast(&mut g,0,dead, "B");
 let mut branch=g.clone();for (case,g) in [("original",&mut g),("branch",&mut branch)]{g.act(0,Move::Choose(Answer::Objects(vec![big]))).unwrap();cite!(["608.2h"]; case: case; assert_eq!(g.players[0].life,16));let copy=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(1)).unwrap();assert_eq!(g.power(copy),Some(6));}
}
#[test]
fn blocked_entry_still_reads_the_original_cards_mana_value(){
 let death=CardDef::new("Death","{1}{B}","Sorcery","Return target creature card from your graveyard to the battlefield. You lose life equal to its mana value.",None).unwrap();
 let body=CardDef::new("Body","{2}{R}","Creature — Elf","",Some((1,1))).unwrap();
 let mut g=Game::new(vec![death,body,pinned("Grafdigger's Cage")],3,20);g.put(1,2,Zone::Battlefield);let dead=g.put(0,1,Zone::Graveyard);cast(&mut g,0,dead, "WB");
 assert!(g.players[0].graveyard.contains(&dead));cite!(["608.2h"]; assert_eq!(g.players[0].life,17));
}
#[test]
fn sister_hospitaller_gains_the_original_clone_cards_mana_value(){
 let mut g=Game::new(vec![pinned("Sister Hospitaller"),pinned("Clone"),big()],3,20);let big=g.put(1,2,Zone::Battlefield);let dead=g.put(0,1,Zone::Graveyard);
 g.players[0].pool.colors=[10;5];let c=g.put(0,0,Zone::Hand);g.act(0,paid("WWWWWB", Move::Cast{card:c,targets:vec![]})).unwrap();pass(&mut g);
 g.act(0,Move::ChooseTargets(vec![vec![Target::Object(dead)]])).unwrap();pass(&mut g);g.act(0,Move::Choose(Answer::Objects(vec![big]))).unwrap();cite!(["608.2h"]; assert_eq!(g.players[0].life,24));
}
#[test]
fn creature_damage_clause_reads_the_returned_copys_power(){
 let effect=CardDef::new("Return and damage","{0}","Sorcery","Return target creature card from your graveyard to the battlefield. That creature deals damage equal to its power to you.",None).unwrap();game::play::check(&effect).unwrap();
 let mut g=Game::new(vec![effect,pinned("Clone"),big()],3,20);let big=g.put(1,2,Zone::Battlefield);let dead=g.put(0,1,Zone::Graveyard);cast(&mut g,0,dead, "");
 g.act(0,Move::Choose(Answer::Objects(vec![big]))).unwrap();cite!(["400.7j"]; assert_eq!(g.players[0].life,14,"returned 6/6 deals six, not the original Clone's zero power"));
 let copy=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(1)).unwrap();cite!(["707.2"]; assert_eq!(g.power(copy),Some(6)));
}

#[test]
fn part_in_friendship_otherwise_uses_the_original_revealed_card(){
 for battlefield in [false,true]{
  let body=CardDef::new("Body","{2}{G}","Creature — Elf","",Some((1,1))).unwrap();
  let kill=CardDef::new("Kill","{0}","Instant","Destroy target creature.",None).unwrap();
  let mut g=Game::new(vec![pinned("Part in Friendship"),body,kill,pinned("Forest")],3,20);
  g.put(0,0,Zone::Battlefield);let dying=g.put(0,1,Zone::Battlefield);g.put(0,1,Zone::Library);
  if battlefield{for _ in 0..3{g.put(0,3,Zone::Battlefield);}}
  cast(&mut g,2,dying, "");assert_eq!(g.stack.len(),1);pass(&mut g);assert!(g.stack.is_empty());
  if battlefield{assert!(g.battlefield.iter().any(|o|g.obj(*o).card==Some(1)));assert!(g.players[0].hand.is_empty());}
  else{assert!(g.players[0].hand.iter().any(|o|g.obj(*o).card==Some(1)));assert!(!g.battlefield.iter().any(|o|g.obj(*o).card==Some(1)));}
 }
}
