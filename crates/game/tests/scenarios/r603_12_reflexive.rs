//! A completed local payment creates a new occurrence, targets only after resolution.
use crate::support::{cite, mana_pool, paid};
use game::{CardDef,Game,Move,Target,Zone};
use game::state::{Answer,Choice,Decision};
fn card(name:&str,kind:&str,text:&str,pt:Option<(i32,i32)>)->CardDef{CardDef::new(name,"{0}",kind,text,pt).unwrap()}
fn setup(text:&str)->Game{
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let catalog=CardDef::catalog(&faces);
 let pinned=|name:&str|CardDef::from_faces(catalog.get(name).unwrap()).unwrap();
 let defs=vec![card("Parent","Instant",text,None),card("Body","Creature","",Some((1,1))),
 pinned("Labyrinth Adversary"),pinned("Glorifier of Suffering"),pinned("Delney, Streetwise Lookout"),
 card("Witness","Creature","Whenever a creature you control dies, you gain 1 life.",Some((2,2))),
 card("Prevent","Enchantment","",None)];
 for d in &defs{game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}Game::new(defs,3,20)
}
fn pass(g:&mut Game){for _ in 0..3{g.act(g.priority,Move::Pass).unwrap();}}
fn cast(g:&mut Game,def:usize){let c=g.put(0,def,Zone::Hand);g.act(0,Move::Cast{card:c,targets:vec![]}).unwrap();pass(g);}
fn yes(g:&mut Game){g.act(0,Move::Choose(Answer::Yes)).unwrap();}
fn pay(g:&mut Game){g.act(0,Move::Choose(Answer::Pay { plan: vec![], with: None, tapped: vec![] })).unwrap();}
fn targets(g:&mut Game,ts:Vec<Vec<Target>>){g.act(0,Move::ChooseTargets(ts)).unwrap();}
#[test]
fn zero_payment_decline_and_fresh_targets_after_parent_finishes(){
 for accept in [false,true]{let mut g=setup("Create a 1/1 white Soldier creature token. You may pay {0}. When you do, target creature can't block this turn.");
  cast(&mut g,0);assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::Pay{..},..})));
  assert_eq!(g.stack.len(),1);assert_eq!(g.battlefield.len(),1);
  crate::support::refused(g.act(1,Move::Choose(Answer::Yes)), "that decision is another player's");let mut clone=g.clone();
  for g in [&mut g,&mut clone]{g.act(0,Move::Choose(if accept{Answer::Pay { plan: vec![], with: None, tapped: vec![] }}else{Answer::No})).unwrap();
   assert_eq!(g.battlefield.len(),1);let token=g.battlefield[0];
   if accept{assert!(matches!(g.decision,Some(Decision::Targets{..})));targets(g,vec![vec![Target::Object(token)]]);cite!(["603.12"]; assert_eq!(g.stack.len(),1));pass(g);}
   else{assert!(g.decision.is_none());cite!(["603.12"]; assert!(g.stack.is_empty()));}
  }
 }
}
#[test]
fn full_adversary_parent_does_not_target_and_success_has_response_window(){
 let mut g=setup("You gain 1 life.");let source=g.put(0,2,Zone::Battlefield);let other=g.put(1,1,Zone::Battlefield);
 pass(&mut g);pass(&mut g);g.act(0,Move::Attack(vec![(source,Target::Player(1))])).unwrap();
 assert_eq!(g.stack.len(),1);g.players[0].pool.colors[3]=2;pass(&mut g);g.act(0,Move::Choose(Answer::Pay{plan:vec![],with:Some(mana_pool("RR")),tapped:vec![]})).unwrap();
 assert!(matches!(g.decision,Some(Decision::Targets{..})));targets(&mut g,vec![vec![Target::Object(other)]]);
 cite!(["603.12"]; assert_eq!(g.stack.len(),1));assert!(g.decision.is_none());pass(&mut g);
}
#[test]
fn full_glorifier_actual_sacrifice_source_departure_and_no_legal_payment(){
 for prevented in [false,true]{let mut g=setup("You gain 1 life.");let a=(!prevented).then(||g.put(0,1,Zone::Battlefield));let b=(!prevented).then(||g.put(0,1,Zone::Battlefield));
  if prevented{g.put(0,6,Zone::Battlefield);}g.players[0].pool.colors[0]=3;let c=g.put(0,3,Zone::Hand);g.act(0,paid("WWW",Move::Cast{card:c,targets:vec![]})).unwrap();pass(&mut g);pass(&mut g);
  if prevented{assert!(g.decision.is_none());assert!(g.stack.is_empty());continue;}
  assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::May { .. },..})));yes(&mut g);
  g.act(0,Move::Choose(Answer::Objects(vec![a.unwrap()]))).unwrap();assert!(matches!(g.decision,Some(Decision::Targets{..})));
  let source=*g.battlefield.last().unwrap();g.move_object(source,Zone::Graveyard);let mut clone=g.clone();
  for g in [&mut g,&mut clone]{targets(g,vec![vec![Target::Object(b.unwrap())]]);assert_eq!(g.obj(b.unwrap()).counters.get("+1/+1"),None);pass(g);cite!(["603.12", "113.7a"]; assert_eq!(g.obj(b.unwrap()).counters.get("+1/+1"),Some(&1)));}
 }
}
#[test]
fn ordinary_modifiers_duplicate_parents_but_not_their_reflexive_occurrences(){
 let mut g=setup("You gain 1 life.");g.put(0,4,Zone::Battlefield);
 // Small source: Delney duplicates the ordinary cast watcher, but each completed zero payment creates exactly one reflexive occurrence.
 let watcher=card("Small","Creature","Whenever you cast a spell, you may pay {0}. When you do, you gain 2 life.",Some((1,1)));
 let mut defs=(*g.defs).clone();defs.push(watcher);let mut g=Game::new(defs,3,20);g.put(0,4,Zone::Battlefield);g.put(0,7,Zone::Battlefield);
 let c=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card:c,targets:vec![]}).unwrap();
 if matches!(g.decision,Some(Decision::Order{..})){g.act(0,Move::Order(vec![0,1])).unwrap();}
 cite!(["603.2d"]; assert_eq!(g.stack.len(),3));
 for _ in 0..2{pass(&mut g);pay(&mut g);assert!(g.decision.is_none());pass(&mut g);}
 cite!(["603.2d"]; assert_eq!(g.players[0].life,24));pass(&mut g);assert_eq!(g.players[0].life,25);
}
#[test]
fn unsupported_producers_and_repeated_payments_stay_refused(){
 for text in ["When you do, you gain 1 life.","Surveil 2. When you do, you gain 1 life.","You may pay {1} any number of times. When you do, you gain 1 life.","Target player may pay {0}. When you do, you gain 1 life."]{
  // Uncited (admission): a repeated payment is read but not lowered; other rows name missing producer scope.
  let d=CardDef::new("Refused","{0}","Instant",text,None).unwrap();
  let reason=if text.contains("any number") {"action not lowered: AnyNumberOfTimes"} else if text.starts_with("Target") {"reflexive: producer must be this resolution’s controller"} else {"reflexive: no earlier payment producer"};
  crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
 }
}

#[test]
fn impossible_mana_and_explicit_zero_payment_have_distinct_outcomes(){
 let mut g=setup("You may pay {1}. When you do, you gain 4 life.");cast(&mut g,0);
 assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Pay{..}})));
 let before=format!("{g:?}");
 cite!(["118.3"]; refused("your mana pool cannot pay that cost (118.3a): {1}"): g.act(0,Move::Choose(Answer::Pay { plan:vec![],with:None,tapped:vec![] })));
 assert_eq!(format!("{g:?}"),before);
 g.act(0,Move::Choose(Answer::No)).unwrap();
 assert!(g.decision.is_none());cite!(["603.12"]; assert!(g.stack.is_empty()));assert_eq!(g.players[0].life,20);
 let mut g=setup("You may pay {0}. When you do, you gain 4 life.");cast(&mut g,0);
 g.act(0,Move::Choose(Answer::Pay{plan:vec![],with:None,tapped:vec![]})).unwrap();cite!(["603.12"]; assert_eq!(g.stack.len(),1));assert_eq!(g.players[0].life,20);pass(&mut g);assert_eq!(g.players[0].life,24);
}
#[test]
fn replacement_suspension_clone_and_source_sacrifice_retain_one_occurrence(){
 let g=setup("You may sacrifice a creature. When you do, target player gains 3 life.");
 let mut defs=(*g.defs).clone();defs.push(card("Leader","Legendary Creature — Human","",Some((2,2))));let mut g=Game::new(defs,3,20);
 let leader=g.put(0,7,Zone::Command);g.designate_commander(leader).unwrap();g.move_object(leader,Zone::Battlefield);
 cast(&mut g,0);yes(&mut g);
 assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::CommandZone{..},..})));
 let mut clone=g.clone();
 for (g,answer) in [(&mut g,Answer::Yes),(&mut clone,Answer::No)]{
  crate::support::refused(g.act(1,Move::Choose(answer.clone())), "that decision is another player's");g.act(0,Move::Choose(answer)).unwrap();
  assert!(matches!(g.decision,Some(Decision::Targets{..})));targets(g,vec![vec![Target::Player(2)]]);
  cite!(["603.12"]; assert_eq!(g.stack.len(),1));pass(g);assert_eq!(g.players[2].life,23);assert!(g.stack.is_empty());
 }
 let parent=card("Departed","Creature","{0}: You may sacrifice this creature. When you do, target player gains 3 life.",Some((1,1)));
 let mut g=Game::new(vec![parent],3,20);let source=g.put(0,0,Zone::Battlefield);
 g.act(0,Move::Activate{source,ability:0,targets:vec![],paying:vec![]}).unwrap();pass(&mut g);yes(&mut g);
 assert!(!g.battlefield.contains(&source));targets(&mut g,vec![vec![Target::Player(1)]]);pass(&mut g);cite!(["603.12", "113.7a"]; assert_eq!(g.players[1].life,23));
}

#[test]
fn suspended_failed_payment_reversal_and_clones_never_queue_early(){
 use game::state::Pool;use game::legal::Step;use ir::mana::ManaSymbol;use ir::Color;
 let ds=vec![card("Parent","Instant","You may pay {B}. When you do, you gain 3 life.",None),
 card("Elf","Creature","{T}: You gain 2 life. Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.",Some((1,1))),
 card("Scales","Enchantment","If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",None),
 card("Double","Enchantment","If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.",None)];
 let mut g=Game::new(ds,3,20);let elf=g.put(0,1,Zone::Battlefield);g.put(0,2,Zone::Battlefield);let double=g.put(0,3,Zone::Battlefield);
 g.players[0].pool.colors[2]=1;cast(&mut g,0);
 let step=Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source:elf,ability:0,paying:vec![],adding:vec![vec![ManaSymbol::Color(Color::Black);3]]};
 g.act(0,Move::Choose(Answer::Pay{plan:vec![step],with:Some(Pool{colors:[0,0,5,0,0],..Pool::default()}),tapped:vec![]})).unwrap();
 assert_eq!(g.stack.len(),1);assert_eq!(g.players[0].life,22);
 let Some(Decision::Choose{choice:Choice::Replacement{among,..},..})=&g.decision else{panic!("{:?}",g.decision)};
 let replacement=among.iter().find(|e|e.source==double).unwrap().clone();g.act(0,Move::Choose(Answer::Replacement(replacement))).unwrap();
 assert!(matches!(g.decision,Some(Decision::Payment{..})));let mut clone=g.clone();
 g.act(0,Move::ReversePayment).unwrap();cite!(["603.12"]; assert!(g.stack.is_empty()));assert_eq!(g.players[0].life,22);
 cite!(["118.3a"]; refused("that mana is not in your mana pool (118.3a)"): clone.act(0,Move::FinishPayment{plan:vec![],paying:vec![],with:Some(Pool{colors:[0,0,5,0,0],..Pool::default()})}));
 clone.act(0,Move::FinishPayment{plan:vec![],paying:vec![],with:Some(mana_pool("B"))}).unwrap();cite!(["603.12"]; assert_eq!(clone.stack.len(),1));assert_eq!(clone.players[0].life,22);pass(&mut clone);assert_eq!(clone.players[0].life,25);
}
#[test]
fn same_source_pending_resolutions_keep_distinct_sacrificed_lki(){
 let source=card("Small","Creature","{0}: You may sacrifice another creature. When you do, you gain life equal to that creature's power.",Some((1,1)));
 let ds=vec![source,card("Two","Creature","",Some((2,2))),card("Five","Creature","",Some((5,5)))];
 for d in &ds{game::play::check(d).unwrap();}let mut g=Game::new(ds,3,20);
 let source=g.put(0,0,Zone::Battlefield);let two=g.put(0,1,Zone::Battlefield);let five=g.put(0,2,Zone::Battlefield);
 for _ in 0..2{g.act(0,Move::Activate{source,ability:0,targets:vec![],paying:vec![]}).unwrap();}
 pass(&mut g);yes(&mut g);g.act(0,Move::Choose(Answer::Objects(vec![two]))).unwrap();let mut clone=g.clone();
 for g in [&mut g,&mut clone]{assert_eq!(g.stack.len(),2);pass(g);cite!(["608.2h"]; assert_eq!(g.players[0].life,22));
  pass(g);yes(g);assert!(!g.battlefield.contains(&five));assert_eq!(g.stack.len(),1);pass(g);cite!(["608.2h"]; assert_eq!(g.players[0].life,27));}
}
#[test]
fn quoted_token_death_parent_and_reflexive_body_keep_the_token_definition(){
 let maker=card("Maker","Sorcery","Create a 1/1 white Soldier creature token with \"When this token dies, you may pay {1}. When you do, draw a card.\"",None);
 let destroy=card("Destroy","Instant","Destroy target creature.",None);
 for d in [&maker,&destroy]{game::play::check(d).unwrap();}
 let mut g=Game::new(vec![maker,destroy],3,20);g.put(0,0,Zone::Library);cast(&mut g,0);let token=g.battlefield[0];
 let kill=g.put(0,1,Zone::Hand);g.act(0,Move::Cast{card:kill,targets:vec![vec![Target::Object(token)]]}).unwrap();pass(&mut g);
 assert!(!g.battlefield.contains(&token));cite!(["603.10a"]; assert_eq!(g.stack.len(),1));g.players[0].pool.colorless=1;pass(&mut g);
 let mut clone=g.clone();for g in [&mut g,&mut clone]{g.act(0,Move::Choose(Answer::Pay{plan:vec![],with:Some(mana_pool("C")),tapped:vec![]})).unwrap();assert_eq!(g.players[0].hand.len(),0);assert_eq!(g.stack.len(),1);pass(g);cite!(["603.12"]; assert_eq!(g.players[0].hand.len(),1));}
}

#[test]
fn following_instructions_only_resolve_in_the_reflexive_body(){
 for accept in [false,true]{
  let mut g=setup("You may pay {0}. When you do, you gain 1 life. You gain 2 life.");
  cast(&mut g,0);g.act(0,Move::Choose(if accept{Answer::Pay { plan: vec![], with: None, tapped: vec![] }}else{Answer::No})).unwrap();
  assert_eq!(g.players[0].life,20);cite!(["603.12"]; case: format!("accept {accept}"); assert_eq!(g.stack.len(),usize::from(accept)));
  let mut clone=g.clone();for g in [&mut g,&mut clone]{if accept{pass(g);cite!(["603.12"]; assert_eq!(g.players[0].life,23));}else{assert_eq!(g.players[0].life,20);}}
 }
}
#[test]
fn terror_return_and_phyrexian_rider_share_the_reflexive_body(){
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let catalog=CardDef::catalog(&faces);
 let terror=CardDef::from_faces(catalog.get("Terror of Towashi").unwrap()).unwrap();game::play::check(&terror).unwrap();
 for accept in [false,true]{
  let mut g=Game::new(vec![terror.clone(),card("Body","Creature — Elf","",Some((1,1)))],3,20);
  let source=g.put(0,0,Zone::Battlefield);let dead=g.put(0,1,Zone::Graveyard);
  pass(&mut g);pass(&mut g);g.act(0,Move::Attack(vec![(source,Target::Player(1))])).unwrap();
  g.players[0].pool.colors[2]=4;pass(&mut g);g.act(0,Move::Choose(if accept{Answer::Pay { plan: vec![], with: Some(mana_pool("BBBB")), tapped: vec![] }}else{Answer::No})).unwrap();
  assert_eq!(g.battlefield.len(),1);
  if !accept{cite!(["603.12"]; assert!(g.stack.is_empty()));assert!(g.players[0].graveyard.contains(&dead));continue;}
  targets(&mut g,vec![vec![Target::Object(dead)]]);let mut clone=g.clone();
  for g in [&mut g,&mut clone]{pass(g);let back=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(1)).unwrap();
   let seen=g.seen(back,None);assert!(seen.subtypes.contains(&ir::Subtype::from_name("Phyrexian").unwrap()));assert!(seen.subtypes.contains(&ir::Subtype::from_name("Elf").unwrap()));}
 }
}
#[test]
fn activation_restrictions_stay_in_parent_and_nested_reflexive_scope_is_refused(){
 let d=card("Parent","Creature","{0}: You may pay {0}. When you do, you gain 1 life. Activate only as a sorcery.",Some((1,1)));
 game::play::check(&d).unwrap();let mut g=Game::new(vec![d],3,20);let source=g.put(0,0,Zone::Battlefield);
 g.act(0,Move::Activate{source,ability:0,targets:vec![],paying:vec![]}).unwrap();
 cite!(["602.5d"]; refused("that ability is activated only as a sorcery: in a main phase of your own turn with the stack empty (602.5d)"): g.act(0,Move::Activate{source,ability:0,targets:vec![],paying:vec![]}));pass(&mut g);pay(&mut g);pass(&mut g);cite!(["603.12"]; assert_eq!(g.players[0].life,21));
 let d=card("Nested","Instant","You may pay {0}. If you control a creature, when you do, you gain 1 life. You gain 2 life.",None);
 // Uncited (admission): this nested reflexive scope is explicitly unavailable.
 crate::support::refused(game::play::check(&d).map_err(|e|e.0), "reflexive: nested scope not lowered");
}

#[test]
fn turn_limit_riders_belong_to_parent_not_captured_body(){
 for rider in ["This ability triggers only once each turn.","Do this only once each turn."]{
  let watcher=card("Watcher","Creature",&format!("Whenever you cast a spell, you may pay {{0}}. When you do, you gain 1 life. You gain 2 life. {rider}"),Some((1,1)));
  game::play::check(&watcher).unwrap();let mut g=Game::new(vec![card("Spell","Instant","You gain 1 life.",None),watcher],3,20);g.put(0,1,Zone::Battlefield);
  let spell=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card:spell,targets:vec![]}).unwrap();pass(&mut g);pay(&mut g);
  assert_eq!(g.players[0].life,20);pass(&mut g);cite!(["603.12"]; case: rider; assert_eq!(g.players[0].life,23));pass(&mut g);
  let spell=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card:spell,targets:vec![]}).unwrap();
  while !g.stack.is_empty(){pass(&mut g);assert!(g.decision.is_none());}
  if rider.starts_with("Do this"){cite!(["603.2h"]; case: rider; assert_eq!(g.players[0].life,25));}
  // Uncited (card text): "This ability triggers only once each turn."
  else{assert_eq!(g.players[0].life,25);}
 }
}

#[test]
fn full_spined_tyrranax_counter_and_trample_share_the_selected_target(){
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let catalog=CardDef::catalog(&faces);
 let spined=CardDef::from_faces(catalog.get("Spined Tyrranax").unwrap()).unwrap();game::play::check(&spined).unwrap();
 let mut g=Game::new(vec![spined,card("Body","Creature — Elf","",Some((1,1)))],3,20);
 g.put(0,0,Zone::Battlefield);let body=g.put(0,1,Zone::Battlefield);
 pass(&mut g);assert_eq!(g.stack.len(),1);g.players[0].pool.colors[4]=3;pass(&mut g);g.act(0,Move::Choose(Answer::Pay{plan:vec![],with:Some(mana_pool("GGG")),tapped:vec![]})).unwrap();
 targets(&mut g,vec![vec![Target::Object(body)]]);assert_eq!(g.obj(body).counters.get("+1/+1"),None);
 assert!(!g.has(body,"Trample"));pass(&mut g);cite!(["603.12"]; assert_eq!(g.obj(body).counters.get("+1/+1"),Some(&1)));assert!(g.has(body,"Trample"));
}
