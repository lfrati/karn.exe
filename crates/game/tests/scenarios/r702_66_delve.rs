//! Delve pays locked generic mana by ordinary graveyard exile (702.66a-b).
use crate::support::cite;
use game::{CardDef,Game,Move,ObjId,Zone};
use game::simulation::Simulation;
fn defs()->Vec<CardDef>{
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
 let cruise=CardDef::from_face(faces.iter().find(|f|f.name=="Treasure Cruise").unwrap()).unwrap();
 let d=vec![cruise,CardDef::new("Bear","{0}","Creature — Bear","",Some((2,2))).unwrap(),
 CardDef::new("Tax","{0}","Enchantment","Noncreature spells cost {1} more to cast.",None).unwrap(),
 CardDef::new("Trinisphere","{3}","Artifact","As long as Trinisphere is untapped, each spell that would cost less than three mana to cast costs three mana to cast.",None).unwrap(),
 CardDef::new("Minimum witness","{0}","Sorcery","Delve\nYou gain 1 life.",None).unwrap(),
 CardDef::new("Alternative witness","{7}{U}","Sorcery","Delve\nYou may pay 2 life rather than pay this spell's mana cost.\nAs an additional cost to cast this spell, discard a card.\nYou gain 1 life.",None).unwrap()];
 for d in &d{game::play::check(d).unwrap_or_else(|e|panic!("{}: {e:?}",d.name()));}d
}
fn respond(s:&mut Simulation,m:Move){let r=s.request().unwrap().unwrap().clone();s.respond(r.id,r.player,m).unwrap();}
fn resolve(s:&mut Simulation){for _ in 0..s.game().unwrap().players.len(){respond(s,Move::Pass);}}
fn cast(card:ObjId,n:u32,cards:Vec<ObjId>)->Move{Move::WithDelve(n,Box::new(Move::WithCosts(if n>0{vec![cards]}else{vec![]},Box::new(Move::Cast{card,targets:vec![]}))))}
/// The answer, after checking that it changed nothing.
#[track_caller]
fn attempt(s:&mut Simulation,m:Move)->Result<(),game::simulation::ResponseError>{let r=s.request().unwrap().unwrap().clone();let before=format!("{:?}",s.game().unwrap());let out=s.respond(r.id,r.player,m).map(|_|());assert_eq!(format!("{:?}",s.game().unwrap()),before);out}
/// The game refuses this answer for `why` and nothing changes.
#[track_caller]
fn refuse(s:&mut Simulation,m:Move,why:&str){crate::support::refused(attempt(s,m),why);}
#[test]
fn r702_66_cruise_legal_payments_exile_only_enough_for_generic_and_draw_three(){
 for ordinary in [false,true]{let mut g=Game::new(defs(),2,20);let card=g.put(0,0,Zone::Hand);let exiled:Vec<_>=(0..7).map(|_|g.put(0,1,Zone::Graveyard)).collect();for _ in 0..3{g.put(0,1,Zone::Library);}g.players[0].pool.colors[1]=1;if ordinary{g.players[0].pool.colorless=7;}
  for count in 0..=7 {
   let mut supplied = crate::support::mana_pool("U");
   supplied.colorless = if ordinary { 7 - count } else { 0 };
   let result = g.clone().act(0, Move::Paying(supplied, Box::new(cast(card, count, exiled[..count as usize].to_vec()))));
   cite!(["702.66a"]; assert_eq!(result.is_ok(), ordinary || count == 7, "ordinary={ordinary}, delve={count}: {result:?}"));
  }
  let mut sim = Simulation::new(g);

  respond(&mut sim,crate::support::paid(if ordinary { "CCCCCCCU" } else { "U" },cast(card,if ordinary{0}else{7},exiled)));cite!(["702.66a"]; case: format!("ordinary {ordinary}"); assert_eq!(sim.game().unwrap().exile.len(),if ordinary{0}else{7}));assert_eq!(sim.game().unwrap().players[0].pool.total(),0);resolve(&mut sim);assert_eq!(sim.game().unwrap().players[0].hand.len(),3);
 }
}
#[test]
fn r702_66_delve_rejects_colored_payment_wrong_zones_owners_duplicates_and_overpayment(){
 let mut g=Game::new(defs(),2,20);let card=g.put(0,0,Zone::Hand);let grave:Vec<_>=(0..8).map(|_|g.put(0,1,Zone::Graveyard)).collect();let hand=g.put(0,1,Zone::Hand);let other=g.put(1,1,Zone::Graveyard);
 // one card for each generic mana: an eighth pays nothing (702.66a); with no mana, seven leave
 // the {U} unpaid
 let unpaid="your mana pool cannot pay that cost (118.3a): {7}{U}";
 let over="delve exiles at most 7 card(s), one for each generic mana of the total cost (702.66a)";
 let mut sim=Simulation::new(g);refuse(&mut sim,cast(card,8,grave.clone()),over);refuse(&mut sim,cast(card,7,grave[..7].to_vec()),unpaid);
 let mut g=sim.game().unwrap().clone();g.players[0].pool.colors[1]=1;let mut sim=Simulation::new(g);
 // a card in hand, an opponent's card, the spell itself: not what the cost asks for; nor a card twice
 for (wrong,why) in [(hand,"Bear is not what that cost asks for or cannot pay it (701.13a, 118.3)"),(other,"Bear is not what that cost asks for or cannot pay it (701.13a, 118.3)"),
  (card,"Treasure Cruise is not what that cost asks for or cannot pay it (701.13a, 118.3)"),(grave[1],"a card is chosen once for that cost (601.2h)")]{let mut paid=grave[..7].to_vec();paid[0]=wrong;cite!(["702.66a"]; case: why; refused(why): attempt(&mut sim,cast(card,7,paid)));}
 // eight for {7}: the {U} is in the pool, so only the overpayment is wrong
 refuse(&mut sim,cast(card,8,grave.clone()),over);respond(&mut sim,crate::support::paid("U",cast(card,7,grave[..7].to_vec())));
}
#[test]
fn r702_66b_taxes_and_final_cost_floors_apply_before_delve_substitution(){
 for floor in [false,true]{let mut g=Game::new(defs(),2,20);g.put(1,if floor{3}else{2},Zone::Battlefield);let card=g.put(0,if floor{4}else{0},Zone::Hand);let count=if floor{3}else{8};let cards:Vec<_>=(0..=count).map(|_|g.put(0,1,Zone::Graveyard)).collect();if !floor{g.players[0].pool.colors[1]=1;}
  // the limit is the total cost's generic mana, so one card more than it is refused
  let mut sim=Simulation::new(g);refuse(&mut sim,cast(card,count+1,cards.clone()),&format!("delve exiles at most {count} card(s), one for each generic mana of the total cost (702.66a)"));
  respond(&mut sim,crate::support::paid(if floor { "" } else { "U" },cast(card,count,cards[..count as usize].to_vec())));cite!(["702.66b"]; case: format!("floor {floor}"); assert_eq!(sim.game().unwrap().players[0].pool.total(),0));cite!(["702.66b"]; case: format!("floor {floor}"); assert_eq!(sim.game().unwrap().exile.len(),count as usize));
 }
}
#[test]
fn r702_66b_delve_combines_with_an_alternative_and_its_additional_cost(){
 let mut g=Game::new(defs(),2,20);g.put(1,2,Zone::Battlefield);let card=g.put(0,5,Zone::Hand);let exile=g.put(0,1,Zone::Graveyard);let discard=g.put(0,1,Zone::Hand);let mut sim=Simulation::new(g);
 respond(&mut sim,Move::CastFor(game::announcement::CastCost::Alternative(1),Box::new(Move::WithDelve(1,Box::new(Move::WithCosts(vec![vec![discard],vec![exile]],Box::new(Move::Cast{card,targets:vec![]})))))));
 cite!(["702.66b"]; assert_eq!(sim.game().unwrap().players[0].life,18));cite!(["702.66b"]; assert_eq!(sim.game().unwrap().exile.len(),1));assert_eq!(sim.game().unwrap().players[0].graveyard.len(),1);resolve(&mut sim);assert_eq!(sim.game().unwrap().players[0].life,19);
}
#[test]
fn r702_66_requires_an_effective_delve_ability(){
 let mut g=Game::new(defs(),2,20);let card=g.put(0,1,Zone::Hand);let exile=g.put(0,1,Zone::Graveyard);let mut sim=Simulation::new(g);refuse(&mut sim,cast(card,1,vec![exile]),"that spell has no delve payment (702.66)");
}
