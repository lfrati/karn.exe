//! CR 702.138a/118.9/205.2: escape is a distinct casting permission with an
//! alternative cost, and a collective constraint counts the selected cards' types.
use crate::support::cite;
use game::{CardDef,Game,Move,ObjId,Zone};
use game::announcement::{CastCost,CastRoute};
use game::simulation::Simulation;
fn defs()->Vec<CardDef>{
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
 let goyf=CardDef::from_face(faces.iter().find(|f|f.name=="Nethergoyf").unwrap()).unwrap();
 let d=vec![goyf,
 CardDef::new("Dual","{0}","Artifact Creature — Bear","",Some((2,2))).unwrap(),
 CardDef::new("Land","","Enchantment Land","",None).unwrap(),
 CardDef::new("Instant","{0}","Instant","You gain 1 life.",None).unwrap(),
 CardDef::new("Sorcery","{0}","Sorcery","You gain 1 life.",None).unwrap(),
 CardDef::new("Tax","{0}","Enchantment","Creature spells cost {1} more to cast.",None).unwrap(),
 CardDef::new("Routes","{0}","Instant","You gain 1 life.\nYou may cast this card from your graveyard.\nFlashback {0}\nEscape—{0}, Exile two other cards from your graveyard.\nYou may pay 2 life rather than pay this spell's mana cost.",None).unwrap(),
 ];for d in &d{game::play::check(d).unwrap_or_else(|e|panic!("{}: {e:?}",d.name()));}d
}
/// Nethergoyf's escape asks for four card types among the other cards exiled (702.138a).
const TYPES:&str="that selection does not satisfy the cost: the chosen cards have fewer than 4 card types among them (601.2h, 701.13a)";
/// A card the cost can't take: another player's, one in hand, or the escaping card itself.
fn not_asked(name:&str)->String{format!("{name} is not what that cost asks for or cannot pay it (701.13a, 118.3)")}
const ONCE:&str="a card is chosen once for that cost (601.2h)";
fn respond(s:&mut Simulation,m:Move){let r=s.request().unwrap().unwrap().clone();s.respond(r.id,r.player,m).unwrap();}
fn passes(s:&mut Simulation){for _ in 0..s.game().unwrap().players.len(){respond(s,Move::Pass);}}
fn cast(card:ObjId,paid:Vec<ObjId>)->Move{Move::CastVia(CastRoute::Escape,Box::new(Move::WithCosts(vec![paid],Box::new(Move::Cast{card,targets:vec![]}))))}
/// The answer, after checking that it changed nothing.
#[track_caller]
fn attempt(s:&mut Simulation,m:Move)->Result<(),game::simulation::ResponseError>{let r=s.request().unwrap().unwrap().clone();let before=format!("{:?}",s.game().unwrap());let out=s.respond(r.id,r.player,m).map(|_|());assert_eq!(format!("{:?}",s.game().unwrap()),before);out}
/// The game refuses this answer for `why` and nothing changes.
#[track_caller]
fn refuse(s:&mut Simulation,m:Move,why:&str){crate::support::refused(attempt(s,m),why);}

#[test]
fn r702_138a_two_multitype_cards_or_any_larger_group_can_pay_escape(){
 for extra in [0,1,5]{let mut g=Game::new(defs(),2,20);let card=g.put(0,0,Zone::Graveyard);let mut paid=vec![g.put(0,1,Zone::Graveyard),g.put(0,2,Zone::Graveyard)];for _ in 0..extra{paid.push(g.put(0,1,Zone::Graveyard));}g.players[0].pool.colors[2]=3;
  let mut sim=Simulation::new(g);
  let command = crate::support::paid("BBB",cast(card,paid));
  respond(&mut sim,command);cite!(["702.138a"]; assert_eq!(sim.game().unwrap().exile.len(),extra+2));assert_eq!(sim.game().unwrap().players[0].pool.total(),0);passes(&mut sim);let goyf=sim.game().unwrap().battlefield[0];cite!(["604.3"]; assert_eq!((sim.game().unwrap().power(goyf),sim.game().unwrap().toughness(goyf)),(Some(0),Some(1))));
 }
}
#[test]
fn r702_138a_listing_and_commands_reject_missing_types_not_just_missing_cards(){
 let mut g=Game::new(defs(),3,20);let card=g.put(0,0,Zone::Graveyard);let own:Vec<_>=(0..5).map(|_|g.put(0,1,Zone::Graveyard)).collect();let other=g.put(2,2,Zone::Graveyard);let hand=g.put(0,2,Zone::Hand);g.players[0].pool.colors[2]=3;let mut sim=Simulation::new(g);
 for (paid,why) in [(own.clone(),TYPES.to_string()),(vec![own[0],other],not_asked("Land")),(vec![own[0],hand],not_asked("Land")),(vec![card,own[0]],not_asked("Nethergoyf")),(vec![own[0],own[0]],ONCE.to_string())]{cite!(["601.2h"]; case: &why; refused(&why): attempt(&mut sim,cast(card,paid)));}
 // Enchantment + Land + Instant become four types only if the escaping creature counts itself.
 let mut g=Game::new(defs(),2,20);let card=g.put(0,0,Zone::Graveyard);
 let land=g.put(0,2,Zone::Graveyard);let instant=g.put(0,3,Zone::Graveyard);
 g.players[0].pool.colors[2]=3;let mut sim=Simulation::new(g);
 cite!(["601.2h"]; refused(&not_asked("Nethergoyf")): attempt(&mut sim,cast(card,vec![card,land,instant])));
}
#[test]
fn r604_3_nethergoyf_counts_only_its_owners_graveyard_in_all_zones(){
 let mut g=Game::new(defs(),3,20);let goyfs=[Zone::Hand,Zone::Library,Zone::Graveyard,Zone::Exile,Zone::Battlefield].map(|z|g.put(0,0,z));g.put(0,1,Zone::Graveyard);g.put(0,2,Zone::Graveyard);g.put(1,3,Zone::Graveyard);g.put(2,4,Zone::Graveyard);
 for goyf in goyfs{cite!(["604.3"]; assert_eq!((g.power(goyf),g.toughness(goyf)),(Some(4),Some(5))));}
}
#[test]
fn r118_9_escape_retains_taxes_timing_and_ownership_but_normal_cast_uses_printed_cost(){
 let mut g=Game::new(defs(),2,20);g.put(1,5,Zone::Battlefield);let card=g.put(0,0,Zone::Graveyard);let paid=vec![g.put(0,1,Zone::Graveyard),g.put(0,2,Zone::Graveyard)];g.players[0].pool.colors[2]=3;let mut sim=Simulation::new(g);cite!(["118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): attempt(&mut sim,crate::support::paid("BBB",cast(card,paid.clone()))));
 let mut g=sim.game().unwrap().clone();g.players[0].pool.colors[2]=4;let mut sim=Simulation::new(g);respond(&mut sim,crate::support::paid("BBBB",cast(card,paid)));cite!(["118.9d"]; assert_eq!(sim.game().unwrap().players[0].pool.total(),0));
 let mut g=Game::new(defs(),2,20);let hand=g.put(0,0,Zone::Hand);
 let paid=vec![g.put(0,1,Zone::Graveyard),g.put(0,2,Zone::Graveyard)];
 g.players[0].pool.colors[2]=3;let mut sim=Simulation::new(g);
 refuse(&mut sim,cast(hand,paid),"that card cannot be cast using escape (702.138a)");respond(&mut sim,crate::support::paid("B",Move::Cast{card:hand,targets:vec![]}));
 cite!(["601.2f"]; assert_eq!(sim.game().unwrap().players[0].pool.total(),2,"normal casting pays only the printed cost"));passes(&mut sim);
 let mut g=Game::new(defs(),2,20);g.active=1;let card=g.put(0,0,Zone::Graveyard);let paid=vec![g.put(0,1,Zone::Graveyard),g.put(0,2,Zone::Graveyard)];g.players[0].pool.colors[2]=3;let mut sim=Simulation::new(g);cite!(["117.1a"]; refused("that is cast in a main phase of your own turn with the stack empty (117.1a)"): attempt(&mut sim,cast(card,paid)));
}
#[test]
fn r118_9_escape_and_flashback_are_exclusive_routes_but_normal_graveyard_permission_coexists(){
 let mut g=Game::new(defs(),2,20);let card=g.put(0,6,Zone::Graveyard);let paid=vec![g.put(0,1,Zone::Graveyard),g.put(0,2,Zone::Graveyard)];for route in [CastRoute::Normal,CastRoute::Flashback,CastRoute::Escape] {
  let action = Move::CastVia(route, Box::new(Move::WithCosts(if route == CastRoute::Escape { vec![paid.clone()] } else { vec![] }, Box::new(Move::Cast { card, targets: vec![] }))));
  cite!(["702.138a", "702.34a"]; ok: g.clone().act(0, action));
 }
 let mut sim=Simulation::new(g);
 refuse(&mut sim,Move::CastFor(CastCost::Alternative(4),Box::new(cast(card,paid.clone()))),"only one alternative cost is paid for a spell (118.9a)");respond(&mut sim,cast(card,paid));passes(&mut sim);assert_eq!(sim.game().unwrap().players[0].graveyard.len(),1);cite!(["702.138a"]; assert_eq!(sim.game().unwrap().exile.len(),2));
}
#[test]
fn r702_138a_illegal_cost_preserves_graveyard_and_mana_for_a_later_valid_cast(){
 let mut g=Game::new(defs(),2,20);let card=g.put(0,0,Zone::Graveyard);let paid=vec![g.put(0,1,Zone::Graveyard),g.put(0,2,Zone::Graveyard)];g.players[0].pool.colors[2]=3;let mut sim=Simulation::new(g);cite!(["601.2h"]; refused(TYPES): attempt(&mut sim,cast(card,vec![paid[0]])));cite!(["601.2h"]; refused(ONCE): attempt(&mut sim,cast(card,vec![paid[0],paid[1],paid[1]])));respond(&mut sim,crate::support::paid("BBB",cast(card,paid)));
}
#[test]
fn r118_10_collective_type_costs_do_not_admit_unsupported_shared_resource_compositions(){
 let d=CardDef::new("Compound","{0}","Creature — Bear","As an additional cost to cast this spell, discard a card.\nEscape—{2}{B}, Exile any number of other cards from your graveyard with four or more card types among them.",Some((2,2))).unwrap();crate::support::refused(game::play::check(&d).map_err(|e|e.0), "cost: a collective type selection combined with another object cost");
}
#[test]
fn r733_1_escape_cost_is_retained_across_mana_replacement_correction_and_reversal(){
 let mut d=defs();d.push(CardDef::new("Mana creature","{0}","Creature — Elf","{T}: You gain 2 life. Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.",Some((1,1))).unwrap());
 d.push(CardDef::new("Scales","{0}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",None).unwrap());
 d.push(CardDef::new("Evolution","{0}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on it instead.",None).unwrap());
 d.push(CardDef::new("Swamp","","Land — Swamp","",None).unwrap());
 let mut g=Game::new(d,2,20);let card=g.put(0,0,Zone::Graveyard);let paid=vec![g.put(0,1,Zone::Graveyard),g.put(0,2,Zone::Graveyard)];g.put(1,5,Zone::Battlefield);let source=g.put(0,7,Zone::Battlefield);g.put(0,8,Zone::Battlefield);let evolution=g.put(0,9,Zone::Battlefield);let swamp=g.put(0,10,Zone::Battlefield);
 let mut sim=Simulation::new(g);let step=game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source,ability:0,paying:vec![],adding:vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Black);3]]};respond(&mut sim,Move::WithMana(vec![step],Box::new(cast(card,paid.clone()))));
 let Some(game::state::Decision::Choose{choice:game::state::Choice::Replacement{among,..},..})=&sim.game().unwrap().decision else{panic!("replacement")};let effect=among.iter().find(|e|e.source==evolution).unwrap().clone();respond(&mut sim,Move::Choose(game::state::Answer::Replacement(effect)));assert!(matches!(sim.game().unwrap().decision,Some(game::state::Decision::Payment{..})));assert!(sim.game().unwrap().exile.is_empty());
 let mut reversed=sim.clone();respond(&mut reversed,Move::ReversePayment);cite!(["733.1"]; assert!(reversed.game().unwrap().players[0].graveyard.contains(&card)));assert!(paid.iter().all(|c|reversed.game().unwrap().players[0].graveyard.contains(c)));assert!(reversed.game().unwrap().stack.is_empty());assert_eq!(reversed.game().unwrap().players[0].pool.total(),3);
 let finish=|paid|Move::FinishPayment{plan:vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source:swamp,ability:0,paying:vec![],adding:vec![]}],paying:vec![paid],with:Some(crate::support::mana_pool("BBBB"))};cite!(["601.2h"]; refused(TYPES): attempt(&mut sim,finish(vec![paid[0]])));respond(&mut sim,finish(paid));assert_eq!(sim.game().unwrap().exile.len(),2);assert_eq!(sim.game().unwrap().players[0].pool.total(),0);passes(&mut sim);
}
#[test]
fn r701_5_a_countered_escaped_spell_returns_to_its_graveyard_and_may_escape_again(){
 let mut g=Game::new(defs(),2,20);let card=g.put(0,0,Zone::Graveyard);let paid=vec![g.put(0,1,Zone::Graveyard),g.put(0,2,Zone::Graveyard)];g.players[0].pool.colors[2]=3;g.act(0,crate::support::paid("BBB",cast(card,paid))).unwrap();let spell=g.stack[0];assert!(g.counter(spell));let card=g.players[0].graveyard[0];let paid=vec![g.put(0,1,Zone::Graveyard),g.put(0,2,Zone::Graveyard)];g.players[0].pool.colors[2]=3;g.act(0,crate::support::paid("BBB",cast(card,paid))).unwrap();cite!(["702.138a"]; assert_eq!(g.exile.len(),4));
}
#[test]
fn r601_3_cage_blocks_escape_and_only_the_cards_owner_can_escape_it(){
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let cage=CardDef::from_face(faces.iter().find(|f|f.name=="Grafdigger's Cage").unwrap()).unwrap();let mut d=defs();d.push(cage);
 for forbidden in [false,true]{let mut g=Game::new(d.clone(),3,20);let card=g.put(if forbidden{0}else{1},0,Zone::Graveyard);let paid=vec![g.put(0,1,Zone::Graveyard),g.put(0,2,Zone::Graveyard)];if forbidden{g.put(2,7,Zone::Battlefield);}g.players[0].pool.colors[2]=3;let mut sim=Simulation::new(g);
  // Cage prohibits casting from a graveyard (601.3); without it, the card is another player's
  refuse(&mut sim,cast(card,paid),if forbidden{"an effect prohibits casting that spell (601.3)"}else{"that card cannot be cast using escape (702.138a)"});}
}

#[test]
fn r702_138a_multiple_escape_cost_choices_remain_explicitly_refused(){
 let d=CardDef::new("Multiple","{0}","Creature — Bear","Escape—{B}, Exile two other cards from your graveyard.\nEscape—{G}, Exile three other cards from your graveyard.",Some((2,2))).unwrap();
 crate::support::refused(game::play::check(&d).map_err(|e|e.0), "escape: choosing among multiple escape costs");
}
