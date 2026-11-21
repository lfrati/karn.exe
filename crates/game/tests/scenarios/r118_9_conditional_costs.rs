use crate::support::{cite, mana_pool, paid};
use game::{CardDef,Game,Move,ObjId,Target,Zone};
use game::announcement::{CastCost,CastRoute};
use game::simulation::Simulation;
const DAZE:&str="You may return an Island you control to its owner's hand rather than pay this spell's mana cost.\nCounter target spell unless its controller pays {1}.";
const FORCE:&str="If it's not your turn, you may exile a blue card from your hand rather than pay this spell's mana cost.\nCounter target noncreature spell. If that spell is countered this way, exile it instead of putting it into its owner's graveyard.";
const SNUFF:&str="If you control a Swamp, you may pay 4 life rather than pay this spell's mana cost.\nDestroy target nonblack creature. It can't be regenerated.";
const WILL:&str="You may pay 1 life and exile a blue card from your hand rather than pay this spell's mana cost.\nCounter target spell.";
fn defs()->Vec<CardDef>{
 let v=vec![
  CardDef::new("Daze","{1}{U}","Instant",DAZE,None).unwrap(),
  CardDef::new("Force of Negation","{1}{U}{U}","Instant",FORCE,None).unwrap(),
  CardDef::new("Snuff Out","{3}{B}","Instant",SNUFF,None).unwrap(),
  CardDef::new("Force of Will","{3}{U}{U}","Instant",WILL,None).unwrap(),
  CardDef::new("Underground Sea","","Land — Island Swamp","({T}: Add {U} or {B}.)",None).unwrap(),
  CardDef::new("Blue witness","{0}","Instant","You gain 2 life.",None).unwrap(),
  CardDef::new("Bear","{0}","Creature — Bear","",Some((2,2))).unwrap(),
  CardDef::new("Tax","{0}","Enchantment","Noncreature spells cost {1} more to cast.",None).unwrap(),
 ];
 for d in &v {game::play::check(d).unwrap_or_else(|e|panic!("{}: {e:?}",d.name()));} v
}
#[test] fn r118_9_complete_corpus_cards_admit(){defs();}
fn respond(sim:&mut Simulation,m:Move){let r=sim.request().unwrap().unwrap().clone();sim.respond(r.id,r.player,m).unwrap();}
fn resolve(sim:&mut Simulation){for _ in 0..sim.game().unwrap().players.len(){respond(sim,Move::Pass);}}
fn alternative(card:ObjId,resources:Vec<Vec<ObjId>>,target:ObjId)->Move{
 Move::CastVia(CastRoute::Normal,Box::new(Move::CastFor(CastCost::Alternative(0),Box::new(Move::WithCosts(resources,Box::new(Move::Cast{card,targets:vec![vec![Target::Object(target)]]}))))))
}
/// The game refuses this answer for `why` and nothing changes.
#[track_caller]
fn refuse_unchanged(sim:&mut Simulation,m:Move,why:&str){let r=sim.request().unwrap().unwrap().clone();let before=format!("{:?}",sim.game().unwrap());crate::support::refused(sim.respond(r.id,r.player,m),why);assert_eq!(format!("{:?}",sim.game().unwrap()),before);}
fn stack_witness(g:&mut Game)->ObjId{g.priority=1;let c=g.put(1,5,Zone::Hand);g.act(1,Move::Cast{card:c,targets:vec![]}).unwrap();g.act(1,Move::Pass).unwrap();g.stack[0]}
#[test]
fn r118_9_daze_returns_a_controlled_island_to_its_owner_and_offers_the_target_controller_payment(){
 for pay in [false,true]{
  let mut d=defs();d.push(CardDef::new("Control witness","{0}","Instant","Gain control of target permanent.",None).unwrap());
  let mut g=Game::new(d,2,20);g.active=1;
  let island=g.put(1,4,Zone::Battlefield);let control=g.put(0,8,Zone::Hand);
  g.act(0,Move::Cast{card:control,targets:vec![vec![Target::Object(island)]]}).unwrap();for _ in 0..2{g.act(g.priority,Move::Pass).unwrap();}
  let target=stack_witness(&mut g);let card=g.put(0,0,Zone::Hand);
  g.players[1].pool.colorless=1;
  let mut sim=Simulation::new(g);

  respond(&mut sim,alternative(card,vec![vec![island]],target));
  let g=sim.game().unwrap();assert!(!g.battlefield.contains(&island));assert!(g.players[1].hand.iter().any(|o|g.obj(*o).card==Some(4)));
  resolve(&mut sim);
  let r=sim.request().unwrap().unwrap();cite!(["118.12a"]; assert_eq!(r.player,1));assert!(matches!(r.question,game::legal::Asked::Pay{..}));
  respond(&mut sim,Move::Choose(if pay{game::state::Answer::Pay { plan: vec![], with: Some(mana_pool("C")), tapped: vec![] }}else{game::state::Answer::No}));
  if pay{cite!(["118.12a"]; assert_eq!(sim.game().unwrap().stack,vec![target]));resolve(&mut sim);assert_eq!(sim.game().unwrap().players[1].life,22);}else{assert!(sim.game().unwrap().stack.is_empty());assert_eq!(sim.game().unwrap().players[1].pool.total(),1);}
 }
}
#[test]
fn r118_3_daze_cannot_return_an_uncontrolled_island_or_a_card_in_hand(){
 let mut g=Game::new(defs(),2,20);let target=stack_witness(&mut g);let card=g.put(0,0,Zone::Hand);
 let theirs=g.put(1,4,Zone::Battlefield);let hand=g.put(0,4,Zone::Hand);let creature=g.put(0,6,Zone::Battlefield);
 let mut sim=Simulation::new(g);
 for (resource,why) in [(theirs,"Underground Sea is not what that cost asks for"),(hand,"Underground Sea is not what that cost asks for"),
  (creature,"Bear is not what that cost asks for")]{refuse_unchanged(&mut sim,alternative(card,vec![vec![resource]],target),why);}
}
#[test]
fn r601_2f_daze_can_tap_the_returned_island_to_pay_a_tax(){
 let mut g=Game::new(defs(),2,20);let target=stack_witness(&mut g);g.put(1,7,Zone::Battlefield);
 let card=g.put(0,0,Zone::Hand);let island=g.put(0,4,Zone::Battlefield);
 let mut sim=Simulation::new(g);
 let plan=vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source:island, ability:0, paying:vec![], adding:vec![], chose:vec![], cost_branches:None }];
 // the tax is paid on top of the alternative cost (601.2f)
 refuse_unchanged(&mut sim,alternative(card,vec![vec![island]],target),"cannot pay that cost (118.3a): {1} in total, {1} more than it was (601.2f)");
 respond(&mut sim,paid("U", Move::WithMana(plan,Box::new(alternative(card,vec![vec![island]],target)))));
 cite!(["118.9d","601.2f"]; assert_eq!(sim.game().unwrap().players[0].pool.total(),0));assert!(!sim.game().unwrap().battlefield.contains(&island));
 resolve(&mut sim);respond(&mut sim,Move::Choose(game::state::Answer::No));assert!(sim.game().unwrap().stack.is_empty());
}
#[test]
fn r118_9_force_negation_condition_is_any_other_players_turn_and_normal_cost_is_unconditional(){
 for active in 0..3{
  let mut g=Game::new(defs(),3,20);g.active=active;let target=stack_witness(&mut g);g.act(2,Move::Pass).unwrap();
  let card=g.put(0,1,Zone::Hand);let blue=g.put(0,0,Zone::Hand);g.players[0].pool.colors[1]=3;
  let mut sim=Simulation::new(g);

  if active==0 {refuse_unchanged(&mut sim,alternative(card,vec![vec![blue]],target),"the condition for that alternative cost is not true (118.9, 601.2b)");respond(&mut sim,paid("UUU", Move::Cast{card,targets:vec![vec![Target::Object(target)]]}));assert!(sim.game().unwrap().exile.is_empty());}
  else{respond(&mut sim,alternative(card,vec![vec![blue]],target));assert_eq!(sim.game().unwrap().players[0].pool.total(),3);}
  resolve(&mut sim);assert!(sim.game().unwrap().stack.is_empty());
  let g=sim.game().unwrap();assert!(g.exile.iter().any(|o|g.obj(*o).card==Some(5)));assert!(!g.log.iter().any(|e|matches!(e,game::Event::ZoneChanged{was,to:Zone::Graveyard,..}if *was==target)));
 }
}
#[test]
fn r118_3_force_exiles_only_another_blue_card_from_the_casters_hand(){
 for force in [1,3]{
  let mut g=Game::new(defs(),2,20);g.active=1;let target=stack_witness(&mut g);let card=g.put(0,force,Zone::Hand);
  let grave=g.put(0,0,Zone::Graveyard);let theirs=g.put(1,0,Zone::Hand);let colorless=g.put(0,6,Zone::Hand);let blue=g.put(0,0,Zone::Hand);
  let mut sim=Simulation::new(g);
  // the Force itself, a card in the graveyard, the opponent's card, a colorless card
  for wrong in [card,grave,theirs,colorless]{let name=sim.game().unwrap().obj(wrong).chars.name.clone();
   refuse_unchanged(&mut sim,alternative(card,vec![vec![wrong]],target),&format!("{name} is not what that cost asks for or cannot pay it (701.13a, 118.3)"));}
  respond(&mut sim,alternative(card,vec![vec![blue]],target));cite!(["119.4"]; case: format!("force={force}"); assert_eq!(sim.game().unwrap().players[0].life,if force==3{19}else{20}));resolve(&mut sim);
  assert_eq!(sim.game().unwrap().exile.len(),if force==3{1}else{2});
 }
}
#[test]
fn r701_6_force_negation_cannot_exile_an_uncounterable_spell(){
 let mut d=defs();d[5]=CardDef::new("Uncounterable","{0}","Instant","This spell can't be countered.\nYou gain 2 life.",None).unwrap();
 let mut g=Game::new(d,2,20);g.active=1;let target=stack_witness(&mut g);let card=g.put(0,1,Zone::Hand);let blue=g.put(0,0,Zone::Hand);
 let mut sim=Simulation::new(g);respond(&mut sim,alternative(card,vec![vec![blue]],target));resolve(&mut sim);
 cite!(["113.6g"]; assert_eq!(sim.game().unwrap().stack,vec![target]));assert_eq!(sim.game().unwrap().exile.len(),1);resolve(&mut sim);assert_eq!(sim.game().unwrap().players[1].life,22);
}
#[test]
fn r118_9_snuff_out_requires_a_controlled_swamp_and_four_life_but_its_mana_cost_does_not(){
 for (swamp,life) in [(false,20),(true,3),(true,5)]{
  let mut g=Game::new(defs(),2,life);let card=g.put(0,2,Zone::Hand);let bear=g.put(1,6,Zone::Battlefield);g.regenerate(bear);
  g.put(if swamp{0}else{1},4,Zone::Battlefield);g.players[0].pool.colors[2]=4;
  let mut sim=Simulation::new(g);let allowed=swamp&&life>=4;

  if allowed{respond(&mut sim,alternative(card,vec![],bear));cite!(["119.4"]; assert_eq!(sim.game().unwrap().players[0].life,life-4));assert_eq!(sim.game().unwrap().players[0].pool.total(),4);}
  else{refuse_unchanged(&mut sim,alternative(card,vec![],bear),if swamp{"you can't pay 4 life with a life total of 3 (119.4)"}else{"the condition for that alternative cost is not true (118.9, 601.2b)"});respond(&mut sim,paid("BBBB", Move::Cast{card,targets:vec![vec![Target::Object(bear)]]}));}
  resolve(&mut sim);assert!(!sim.game().unwrap().battlefield.contains(&bear));
 }
}
#[test]
fn r115_force_targets_only_noncreature_spells_and_snuff_out_only_nonblack_creatures(){
 let mut d=defs();d.push(CardDef::new("Black bear","{B}","Creature — Bear","",Some((2,2))).unwrap());
 let mut g=Game::new(d,2,20);g.active=1;g.priority=1;let bear=g.put(1,6,Zone::Hand);g.act(1,Move::Cast{card:bear,targets:vec![]}).unwrap();let target=g.stack[0];g.act(1,Move::Pass).unwrap();
 let force=g.put(0,1,Zone::Hand);let snuff=g.put(0,2,Zone::Hand);let blue=g.put(0,0,Zone::Hand);g.put(0,4,Zone::Battlefield);let black=g.put(1,8,Zone::Battlefield);
 let mut sim=Simulation::new(g);refuse_unchanged(&mut sim,alternative(force,vec![vec![blue]],target),"an illegal target, Bear: it is a creature (115.1)");
 refuse_unchanged(&mut sim,alternative(snuff,vec![],black),"an illegal target, Black bear: it is black (115.1)");
}
#[test]
fn r601_2f_a_conditional_cost_stays_locked_when_its_swamp_is_sacrificed_for_tax_mana(){
 let mut d=defs();d.push(CardDef::new("Mana Swamp","","Land — Swamp","{T}, Sacrifice this land: Add {C}.",None).unwrap());
 let mut g=Game::new(d,2,20);let card=g.put(0,2,Zone::Hand);let target=g.put(1,6,Zone::Battlefield);let swamp=g.put(0,8,Zone::Battlefield);g.put(1,7,Zone::Battlefield);
 let mut sim=Simulation::new(g);
 let plan=vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source:swamp,ability:0,paying:vec![],adding:vec![]}];
 respond(&mut sim,paid("C", Move::WithMana(plan,Box::new(alternative(card,vec![],target)))));
 assert!(!sim.game().unwrap().battlefield.contains(&swamp));cite!(["601.2f"]; assert_eq!(sim.game().unwrap().players[0].life,16));resolve(&mut sim);assert!(!sim.game().unwrap().battlefield.contains(&target));
}
#[test]
fn r601_2b_conditional_alternatives_do_not_grant_zone_permission_and_cannot_combine_with_flashback(){
 let mut d=defs();d.push(CardDef::new("Grave witness","{1}","Instant","If it's not your turn, you may pay 2 life rather than pay this spell's mana cost.\nYou gain 3 life.\nYou may cast this card from your graveyard.\nFlashback {1}",None).unwrap());
 game::play::check(d.last().unwrap()).unwrap();
 let mut g=Game::new(d,2,20);g.active=1;let target=stack_witness(&mut g);let force=g.put(0,1,Zone::Graveyard);let blue=g.put(0,0,Zone::Hand);let witness=g.put(0,8,Zone::Graveyard);
 let mut sim=Simulation::new(g);refuse_unchanged(&mut sim,alternative(force,vec![vec![blue]],target),"no rule or effect lets you cast it from there (601.3)");
 let cast=Move::CastFor(CastCost::Alternative(0),Box::new(Move::Cast{card:witness,targets:vec![]}));
 refuse_unchanged(&mut sim,Move::CastVia(CastRoute::Flashback,Box::new(cast.clone())),"only one alternative cost is paid for a spell (118.9a)");
 respond(&mut sim,Move::CastVia(CastRoute::Normal,Box::new(cast)));cite!(["118.9"]; assert_eq!(sim.game().unwrap().players[0].life,18));resolve(&mut sim);assert_eq!(sim.game().unwrap().players[0].life,21);
}
#[test]
fn r601_2b_a_cast_origin_condition_reads_the_announced_cards_zone(){
 let d=CardDef::new("Origin witness","{2}","Instant","If this spell was cast from your hand, you may pay 2 life rather than pay this spell's mana cost.\nYou gain 3 life.\nYou may cast this card from your graveyard.",None).unwrap();game::play::check(&d).unwrap();
 for zone in [Zone::Hand,Zone::Graveyard]{let mut g=Game::new(vec![d.clone()],2,20);let card=g.put(0,0,zone);let mut sim=Simulation::new(g);

  let cast=Move::CastFor(CastCost::Alternative(0),Box::new(Move::Cast{card,targets:vec![]}));
  if zone==Zone::Hand{respond(&mut sim,cast);resolve(&mut sim);assert_eq!(sim.game().unwrap().players[0].life,21);}else{refuse_unchanged(&mut sim,cast,"the condition for that alternative cost is not true (118.9, 601.2b)");}
 }
}
#[test]
fn r733_1_suspended_daze_payment_can_finish_or_reverse_without_returning_the_island_twice(){
 let mut d=defs();d.push(CardDef::new("Mana creature","{0}","Creature — Elf","{T}: You gain 2 life. Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.",Some((1,1))).unwrap());
 d.push(CardDef::new("Scales","{0}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",None).unwrap());
 d.push(CardDef::new("Evolution","{0}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on it instead.",None).unwrap());
 d.push(CardDef::new("Heavy tax","{0}","Enchantment","Noncreature spells cost {4} more to cast.",None).unwrap());
 let mut g=Game::new(d,2,20);let target=stack_witness(&mut g);let card=g.put(0,0,Zone::Hand);let island=g.put(0,4,Zone::Battlefield);let mana=g.put(0,8,Zone::Battlefield);g.put(0,9,Zone::Battlefield);let evolution=g.put(0,10,Zone::Battlefield);g.put(1,11,Zone::Battlefield);
 let mut sim=Simulation::new(g);let plan=vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source:mana,ability:0,paying:vec![],adding:vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Green);3]]}];
 respond(&mut sim,paid("GGGG", Move::WithMana(plan,Box::new(alternative(card,vec![vec![island]],target)))));
 let Some(game::state::Decision::Choose{choice:game::state::Choice::Replacement{among,..},..})=&sim.game().unwrap().decision else{panic!("replacement choice")};let effect=among.iter().find(|e|e.source==evolution).unwrap().clone();
 respond(&mut sim,Move::Choose(game::state::Answer::Replacement(effect)));assert!(matches!(sim.game().unwrap().decision,Some(game::state::Decision::Payment{..})));
 let mut reversed=sim.clone();respond(&mut reversed,Move::ReversePayment);let g=reversed.game().unwrap();cite!(["733.1"]; assert!(g.players[0].hand.contains(&card)));cite!(["733.1"]; assert!(g.battlefield.contains(&island)));assert_eq!(g.stack,vec![target]);assert_eq!(g.players[0].pool.total(),3);assert_eq!(g.players[0].life,22);
 let plan=vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source:island,ability:0,paying:vec![],adding:vec![]}];
 respond(&mut sim,Move::FinishPayment{plan,paying:vec![vec![island]],with:Some(mana_pool("GGGU"))});assert!(!sim.game().unwrap().battlefield.contains(&island));assert_eq!(sim.game().unwrap().players[0].pool.total(),0);resolve(&mut sim);respond(&mut sim,Move::Choose(game::state::Answer::No));assert!(sim.game().unwrap().stack.is_empty());
}
#[test]
fn r601_2f_conditional_variable_life_alternative_announces_and_pays_x(){
 let d=CardDef::new("Variable witness","{1}","Instant","If you control a Swamp, you may pay X life rather than pay this spell's mana cost.\nYou gain 1 life.",None).unwrap();
 game::play::check(&d).unwrap();
 let swamp=CardDef::new("Swamp","","Basic Land — Swamp","",None).unwrap();
 let mut g=Game::new(vec![d,swamp],2,20);let card=g.put(0,0,Zone::Hand);
 let cast=Move::WithX(3,Box::new(Move::CastFor(CastCost::Alternative(0),Box::new(Move::Cast{card,targets:vec![]}))));
 cite!(["118.9", "601.2b"]; refused("the condition for that alternative cost is not true (118.9, 601.2b)"): g.act(0,cast.clone()));assert_eq!(g.players[0].life,20);
 g.put(0,1,Zone::Battlefield);
 g.act(0,cast).unwrap();cite!(["601.2h"]; assert_eq!(g.players[0].life,17));assert_eq!(g.obj(g.stack[0]).x,Some(3));
 for _ in 0..2{g.act(g.priority,Move::Pass).unwrap();}assert_eq!(g.players[0].life,18);
}

#[test]
fn r601_2b_cost_conditions_requiring_unavailable_announcement_facts_are_refused(){
 let d=CardDef::new("Announcement witness","{1}","Instant","Kicker {1}\nIf this spell was kicked, you may pay 2 life rather than pay this spell's mana cost.\nYou gain 1 life.",None).unwrap();
 assert_eq!(game::play::check(&d).expect_err("announcement facts unavailable while alternative is selected").0, "alternative cost condition: announcement/payment facts not yet supplied (601.2b)");
}
