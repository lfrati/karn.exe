//! Completed discard identities and one quantity-scaled follow-up (701.9/608.2i).
use crate::support::cite;
use game::{CardDef,Game,Move,Zone,Event};
use game::state::{Answer,Choice,Decision};
const BODY:&str="Discard two cards, then draw two cards. For each nonland card discarded this way, create a 1/1 red Elemental creature token.";
const FULL:&str="When this creature enters, discard two cards, then draw two cards. For each nonland card discarded this way, create a 1/1 red Elemental creature token.\n{3}{R}{R}, Exile this card from your graveyard: Create two 1/1 red Elemental creature tokens.";
fn def(name:&str,ty:&str,text:&str,pt:Option<(i32,i32)>)->CardDef {CardDef::new(name,"{0}",ty,text,pt).unwrap()}
fn defs(text:&str)->Vec<CardDef>{
 let ds=vec![def("Producer","Sorcery",text,None),def("Bear","Creature — Bear","",Some((2,2))),def("Land","Land","",None),
 def("Replaced","Creature — Bear","If a spell or ability an opponent controls causes you to discard this card, put it onto the battlefield instead of putting it into your graveyard.",Some((2,2))),
 def("Entry restriction","Artifact","Creature cards in hands can't enter the battlefield.",None),
 def("Copy","Creature — Shapeshifter","If a spell or ability an opponent controls causes you to discard this card, put it onto the battlefield instead of putting it into your graveyard.\nYou may have this creature enter as a copy of any permanent on the battlefield.",Some((0,0)))];
 for d in &ds {game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}ds
}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
fn cast(g:&mut Game){let card=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![]}).unwrap();pass(g);}
fn tokens(g:&Game)->usize{g.battlefield.iter().filter(|o|g.obj(**o).token()).count()}
#[test]
fn full_pyromancer_mixed_short_empty_and_graveyard_ability(){
 let d=CardDef::new("Seasoned Pyromancer","{1}{R}{R}","Creature — Human Shaman",FULL,Some((2,2))).unwrap();
 game::play::check(&d).unwrap();
 for hand in [vec![],vec![1],vec![2],vec![1,2],vec![1,1]] {
  let mut ds=defs(BODY);ds[0]=d.clone();let mut g=Game::new(ds,2,20);
  for _ in 0..4 {g.put(0,2,Zone::Library);}
  let card=g.put(0,0,Zone::Hand);let discarded:Vec<_>=hand.iter().map(|index|g.put(0,*index,Zone::Hand)).collect();
  g.players[0].pool.colors[3]=3;g.act(0,crate::support::paid("RRR", Move::Cast{card,targets:vec![]})).unwrap();pass(&mut g);pass(&mut g);
  if hand.len()==2 {cite!(["404.3"]; ok: g.act(0,Move::Choose(Answer::Objects(discarded))));}
  cite!(["608.2c"]; case: format!("{hand:?}"); assert_eq!(tokens(&g),hand.iter().filter(|i|**i==1).count()));cite!(["608.2c"]; case: format!("{hand:?}"); assert_eq!(g.players[0].hand.len(),2));
  let source=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(0)).unwrap();g.destroy(source);
  let card=g.players[0].graveyard.iter().copied().find(|o|g.obj(*o).card==Some(0)).unwrap();
  g.players[0].pool.colors[3]=5;
  g.act(0,crate::support::paid("RRRRR", Move::Activate{source:card,ability:1,targets:vec![],paying:vec![]})).unwrap();pass(&mut g);
  assert_eq!(tokens(&g),hand.iter().filter(|i|**i==1).count()+2);assert!(g.players[0].graveyard.iter().all(|o|g.obj(*o).card!=Some(0)));
 }
}
#[test]
fn private_selection_survives_clone_then_draw_and_counts_completed_not_unselected_cards(){
 let mut g=Game::new(defs(BODY),2,20);for _ in 0..4{g.put(0,1,Zone::Library);}
 let a=g.put(0,1,Zone::Hand);let b=g.put(0,2,Zone::Hand);g.put(0,1,Zone::Hand);cast(&mut g);
 cite!(["701.9b"]; assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Objects{..}}))));
 let mut copy=g.clone();for g in [&mut g,&mut copy]{
  g.retire_events(g.log.end());g.act(0,Move::Choose(Answer::Objects(vec![a,b]))).unwrap();
  cite!(["404.3"]; ok: g.act(0,Move::Choose(Answer::Objects(vec![a,b]))));
  cite!(["608.2c"]; assert_eq!(tokens(g),1));cite!(["608.2c"]; assert_eq!(g.players[0].hand.len(),3));
  assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Discarded{..})).count(),2);
 }
}
#[test]
fn completed_replacement_counts_but_prevented_entry_does_not(){
 for prohibit in [false,true]{
  let mut g=Game::new(defs("Each opponent discards a card. For each nonland card discarded this way, create a 1/1 red Elemental creature token."),2,20);
  let card=g.put(1,3,Zone::Hand);if prohibit{g.put(0,4,Zone::Battlefield);}cast(&mut g);
  cite!(["608.2c", "614.6"]; case: prohibit; assert_eq!(tokens(&g),usize::from(!prohibit)));cite!(["614.6"]; case: prohibit; assert_eq!(g.players[1].hand.contains(&card),prohibit));
 }
}
#[test]
fn replacement_entry_copy_uses_discarded_original_not_new_land_characteristics(){
 let mut g=Game::new(defs("Each opponent discards a card. For each nonland card discarded this way, create a 1/1 red Elemental creature token."),2,20);
 let land=g.put(1,2,Zone::Battlefield);g.put(1,5,Zone::Hand);cast(&mut g);
 assert!(matches!(g.decision,Some(Decision::Choose{player:1,choice:Choice::Select{..}})), "{:?}",g.decision);
 let mut copy=g.clone();for g in [&mut g,&mut copy]{
  g.act(1,Move::Choose(Answer::Objects(vec![land]))).unwrap();cite!(["608.2h"]; assert_eq!(tokens(g),1));
  let entered=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(5)).unwrap();cite!(["707.2"]; assert_eq!(g.def(entered).unwrap().name(),"Land"));
 }
}
#[test]
fn absent_ambiguous_optional_compound_and_member_specific_leads_are_refused(){
 for text in [
 "For each nonland card discarded this way, create a 1/1 red Elemental creature token.",
 "Discard a card. Discard a card. For each nonland card discarded this way, create a 1/1 red Elemental creature token.",
 "Discard a card. For each nonland card discarded this way, you may create a 1/1 red Elemental creature token.",
 "Discard a card. For each nonland card discarded this way, draw a card and gain 1 life.",
 "Discard two cards. For each nonland card discarded this way, create a token that's a copy of that card.",
 "For each creature you control, you gain life equal to that creature's power."]{
  let d=CardDef::new("Refused","{0}","Sorcery",text,None).unwrap();
  let reason=if text.starts_with("For each creature") {"input reference: nothing before it supplies the required participant/cardinality"}
    else if text.starts_with("For each nonland") || text.starts_with("Discard a card. Discard") {"result quantity: absent or ambiguous action-result producer"}
    else {"for-each quantity lead: compound, optional or non-quantity action"};
  crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
 }
}
#[test]
fn completed_original_cards_survive_later_payment_suspension_and_clone(){
 let mut g=Game::new(defs("Discard two cards, then draw two cards. You may pay {1}. For each nonland card discarded this way, create a 1/1 red Elemental creature token."),2,20);
 for _ in 0..4 {g.put(0,1,Zone::Library);}let a=g.put(0,1,Zone::Hand);let b=g.put(0,2,Zone::Hand);g.players[0].pool.colorless=1;cast(&mut g);
 cite!(["404.3"]; ok: g.act(0,Move::Choose(Answer::Objects(vec![a,b]))));
 assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Pay{..}})));
 cite!(["608.2c"]; assert_eq!(g.players[0].hand.len(),2));assert_eq!(tokens(&g),0);
 let mut copy=g.clone();for g in [&mut g,&mut copy]{g.retire_events(g.log.end());g.collect_departed();
  g.act(0,Move::Choose(Answer::No)).unwrap();cite!(["608.2c"]; assert_eq!(tokens(g),1));assert!(g.decision.is_none());}
}
#[test]
fn declined_optional_discard_exports_an_empty_filtered_result(){
 let mut g=Game::new(defs("You may discard a card. For each nonland card discarded this way, create a 1/1 red Elemental creature token."),2,20);
 let card=g.put(0,1,Zone::Hand);cast(&mut g);
 cite!(["608.2d"]; assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::May { possible: true }}))));
 g.act(0,Move::Choose(Answer::No)).unwrap();assert!(g.players[0].hand.contains(&card));cite!(["608.2c"]; assert_eq!(tokens(&g),0));
}
#[test]
fn quantity_lead_creates_one_simultaneous_group_not_separate_replayed_instructions(){
 let mut ds=defs(BODY);ds.push(def("Watcher","Artifact","Whenever a creature enters, if you control two or more creatures, you gain 1 life.",None));
 game::play::check(ds.last().unwrap()).unwrap();let mut g=Game::new(ds,2,20);let watcher=g.put(0,6,Zone::Battlefield);
 for _ in 0..4{g.put(0,2,Zone::Library);}let a=g.put(0,1,Zone::Hand);let b=g.put(0,1,Zone::Hand);cast(&mut g);
 cite!(["404.3"]; ok: g.act(0,Move::Choose(Answer::Objects(vec![a,b]))));
 cite!(["701.7a"]; assert_eq!(tokens(&g),2));
 // Two simultaneous entrants produce two occurrences; 603.3b still requires
 // their controller to order them before either goes onto the stack.
 let Some(Decision::Order{player:0,abilities})=g.decision.clone() else{panic!("expected entry-trigger order: {:?}",g.decision)};
 cite!(["603.3b"]; assert_eq!(abilities.len(),2));
 assert!(abilities.iter().all(|ability|ability.source==Some(watcher)&&ability.controller==0));
 g.act(0,Move::Order(vec![0,1])).unwrap();
 cite!(["603.2", "603.4"]; assert_eq!(g.stack.len(),2));
 pass(&mut g);pass(&mut g);cite!(["603.4"]; assert_eq!(g.players[0].life,22));
}
