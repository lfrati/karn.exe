//! 400.7j: after one card is returned to the battlefield, what the effect does to "it" finds the
//! new permanent, while an amount read from the returned card is its last-known value in the
//! graveyard (Reanimate ruling 5359e380d78d09b1, Sister Hospitaller ruling c0ef686e8865ec1b).
//! An anaphor whose antecedent is the creature ("that creature ... its power") reads the
//! permanent.
use crate::support::{cite, paid};
use game::{CardDef,Game,Move,Target,Zone,ObjId};
use game::state::Answer;
fn pinned(name:&str)->CardDef{let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
 let d=CardDef::from_faces(CardDef::catalog(&faces).get(name).unwrap()).unwrap();game::play::check(&d).unwrap_or_else(|e|panic!("{name}: {}",e.0));d}
fn settle(g:&mut Game){while g.decision.is_none()&&!g.stack.is_empty(){g.act(g.priority,Move::Pass).unwrap();}}
/// defs: 0 spell, 1 a Dragon Elf with mana value 3, 2 Grafdigger's Cage. Returns the game after resolution.
fn reanimate(spell:CardDef,cage:bool,spending:&str)->Game{
 let body=CardDef::new("Body","{2}{R}","Creature — Dragon Elf","",Some((1,1))).unwrap();
 let mut g=Game::new(vec![spell,body,pinned("Grafdigger's Cage")],3,20);
 if cage{g.put(1,2,Zone::Battlefield);}
 let dead=g.put(0,1,Zone::Graveyard);g.players[0].pool.colors=[5,5,5,5,5];
 let c=g.put(0,0,Zone::Hand);g.act(0,paid(spending,Move::Cast{card:c,targets:vec![vec![Target::Object(dead)]]})).unwrap();settle(&mut g);g
}
fn back(g:&Game)->ObjId{g.battlefield.iter().copied().find(|o|g.obj(*o).card==Some(1)).expect("returned")}
fn subtype(n:&str)->ir::Subtype{ir::Subtype::from_name(n).unwrap()}

#[test]
fn object_operands_find_the_returned_permanent(){
 let g=reanimate(pinned("Rise from the Grave"),false,"BBBBB");cite!(["400.7j"]; assert!(g.seen(back(&g),None).subtypes.contains(&subtype("Zombie"))));
 let g=reanimate(pinned("Miraculous Recovery"),false,"WWWWW");cite!(["400.7j"]; assert_eq!(g.obj(back(&g)).counters.get("+1/+1"),Some(&1)));
}
#[test]
fn blocked_return_still_reads_the_cards_mana_value(){
 let death=CardDef::new("Death","{1}{B}","Sorcery","Return target creature card from your graveyard to the battlefield. You lose life equal to its mana value.",None).unwrap();
 for cage in [false,true]{let g=reanimate(death.clone(),cage,"BB");cite!(["608.2h"]; case: format!("cage={cage}"); assert_eq!(g.players[0].life,17,"cage={cage}"));
  assert_eq!(g.players[0].graveyard.iter().any(|o|g.obj(*o).card==Some(1)),cage);}
}
/// Clone enters from the graveyard as a copy of a 6/6 with mana value 6.
fn clone_copy(spell:CardDef,spending:&str)->Game{
 let big=CardDef::new("Big","{5}{G}","Creature — Beast","",Some((6,6))).unwrap();
 let small=CardDef::new("Small","{0}","Creature — Wall","",Some((0,8))).unwrap();
 let mut g=Game::new(vec![spell,pinned("Clone"),big,small],3,20);
 g.put(1,2,Zone::Battlefield);let big_id=g.battlefield[0];let wall=g.put(1,3,Zone::Battlefield);let _=wall;
 let clone=g.put(0,1,Zone::Graveyard);g.players[0].pool.colors=[5,5,5,5,5];
 let c=g.put(0,0,Zone::Hand);g.act(0,paid(spending,Move::Cast{card:c,targets:vec![vec![Target::Object(clone)]]})).unwrap();settle(&mut g);
 g.act(0,Move::Choose(Answer::Objects(vec![big_id]))).unwrap();settle(&mut g);g
}
#[test]
fn reanimate_loses_the_graveyard_cards_mana_value_not_the_copys(){
 let g=clone_copy(pinned("Reanimate"),"B");cite!(["608.2h"]; assert_eq!(g.players[0].life,16));
}
#[test]
fn that_creatures_power_is_the_permanents(){
 // Too Greedily, Too Deep: "That creature deals damage equal to its power to each other creature."
 let g=clone_copy(pinned("Too Greedily, Too Deep"),"WWWWWBR");
 let wall=g.battlefield.iter().copied().find(|o|g.obj(*o).card==Some(3)).unwrap();
 cite!(["400.7j","608.2h"]; assert_eq!(g.obj(wall).damage,6));
}
/// A type condition after the return is checked on the returned creature: Defy Death ruling
/// 55caccc2ce6c10ea (an Angel only on the battlefield gets the counters), Essence Flux ruling
/// aa954da2909056bb, Fearsome Awakening ruling 846231248fee395f.
#[test]
fn type_condition_checks_the_returned_creature(){
 let g=reanimate(pinned("Fearsome Awakening"),false,"BBBBB");cite!(["400.7j"]; assert_eq!(g.obj(back(&g)).counters.get("+1/+1"),Some(&2)));
 // Clone, not a Dragon in the graveyard, enters as a copy of a Dragon
 let dragon=CardDef::new("Big","{5}{G}","Creature — Dragon","",Some((6,6))).unwrap();
 let mut g=Game::new(vec![pinned("Fearsome Awakening"),pinned("Clone"),dragon],3,20);
 g.put(1,2,Zone::Battlefield);let d=g.battlefield[0];let c=g.put(0,1,Zone::Graveyard);g.players[0].pool.colors=[5,5,5,5,5];
 let s=g.put(0,0,Zone::Hand);g.act(0,paid("BBBBB",Move::Cast{card:s,targets:vec![vec![Target::Object(c)]]})).unwrap();settle(&mut g);
 g.act(0,Move::Choose(Answer::Objects(vec![d]))).unwrap();settle(&mut g);
 let b=g.battlefield.iter().copied().find(|o|g.obj(*o).card==Some(1)).unwrap();cite!(["400.7j"]; assert_eq!(g.obj(b).counters.get("+1/+1"),Some(&2)));
}
