//! Replicate's additional cost and independently counterable cast trigger (702.56),
//! with spell copies using the ordinary stack and retained executable identity (707.10).
use crate::support::{cite, mana_pool};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};
use game::event::Event;
use game::play::check;
const TEXT: &str = "Replicate {1}\nCounter target triggered ability or colorless spell.";
fn def(name: &str,cost: &str,types: &str,text: &str) -> CardDef { CardDef::new(name,cost,types,text,None).unwrap() }
fn game() -> Game {
 let defs = vec![def("Consign to Memory","{U}","Instant",TEXT),def("Blank","{0}","Artifact",""),def("Deny","{U}","Instant","Counter target spell."),def("Fork","{0}","Instant","Copy target instant or sorcery spell. You may choose new targets for the copy."),def("Wastes","","Basic Land","{T}: Add {C}.")];
 for d in &defs {check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}
 let mut g=Game::new(defs,2,20);
 for p in 0..2 {for _ in 0..8 {g.put(p,4,Zone::Library);}}
 g.players[0].pool.colors[1]=1;g.players[0].pool.colorless=5;
 g
}
fn pass(g:&mut Game) {g.act(g.priority,Move::Pass).unwrap();g.act(g.priority,Move::Pass).unwrap();}
fn cast(g:&mut Game,card:ObjId,target:ObjId,n:u32) {g.act(g.priority,Move::Paying({ let mut pool = mana_pool("U"); pool.colorless = n; pool }, Box::new(Move::WithReplicate(n,Box::new(Move::Cast{card,targets:vec![vec![Target::Object(target)]]}))))).unwrap();}
fn asking(g:&Game)->ObjId {match &g.decision {Some(Decision::Choose{choice:Choice::Retarget{copy,..},..})=>*copy,other=>panic!("expected copy targets: {other:?}")}}
#[test]
fn r702_56_full_oracle_admits_and_payment_choices_are_listed() {
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let catalog=CardDef::catalog(&faces);
 let real=CardDef::from_faces(catalog.get("Consign to Memory").unwrap()).unwrap();check(&real).unwrap();
 let mut g=game();let target=g.put(1,1,Zone::Stack);let card=g.put(0,0,Zone::Hand);
 cast(&mut g,card,target,0);cite!(["702.56a"]; assert_eq!(g.stack.len(),2));assert!(g.decision.is_none());
}
#[test]
fn r702_56_repeated_cost_is_paid_and_copies_are_not_cast() {
 let mut g=game();let a=g.put(1,1,Zone::Stack);let b=g.put(1,1,Zone::Stack);let card=g.put(0,0,Zone::Hand);
 cast(&mut g,card,a,2);cite!(["702.56a"]; assert_eq!(g.players[0].pool.colorless,3));assert_eq!(g.players[0].pool.colors[1],0);
 assert_eq!(g.stack.len(),4);let original=g.stack[2];let trigger=g.stack[3];cite!(["702.56a"]; assert_eq!(g.obj(trigger).ability.unwrap().0,Some(original)));
 pass(&mut g);let first=asking(&g);assert!(!g.stack.contains(&first));
 g.act(0,Move::Choose(Answer::Targets(vec![vec![Target::Object(b)]]))).unwrap();let second=asking(&g);assert_ne!(first,second);assert!(g.stack.contains(&first));
 let mut clone=g.clone();for g in [&mut g,&mut clone] {g.act(0,Move::Choose(Answer::No)).unwrap();cite!(["702.56a"]; assert_eq!(g.stack.len(),5));cite!(["707.10"]; assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Cast{..})).count(),1));pass(g);pass(g);pass(g);assert!(g.stack.is_empty());assert_eq!(g.players[0].graveyard.len(),1);}
 assert_eq!(g.players[1].graveyard.len(),2);
}
#[test]
fn r702_56_trigger_can_be_countered_and_source_can_be_countered_independently() {
 let mut g=game();let a=g.put(1,1,Zone::Stack);let card=g.put(0,0,Zone::Hand);cast(&mut g,card,a,1);
 let trigger=*g.stack.last().unwrap();assert!(g.counter(trigger));pass(&mut g);assert!(g.stack.is_empty());cite!(["702.56a"]; assert!(g.decision.is_none()));
 let mut g=game();let a=g.put(1,1,Zone::Stack);let card=g.put(0,0,Zone::Hand);cast(&mut g,card,a,1);let original=g.stack[1];assert!(g.counter(original));pass(&mut g);asking(&g);g.act(0,Move::Choose(Answer::No)).unwrap();pass(&mut g);assert!(g.stack.is_empty());
}
#[test]
fn r707_10c_old_illegal_targets_may_stay_but_new_ones_must_be_legal() {
 let mut g=game();let a=g.put(1,1,Zone::Stack);let b=g.put(1,2,Zone::Stack);let card=g.put(0,0,Zone::Hand);cast(&mut g,card,a,1);assert!(g.counter(a));pass(&mut g);
 let copy=asking(&g);let before=g.stack.clone();
 cite!(["707.10c", "115.7d"]; refused("a changed target must be legal without invalidating an unchanged target (115.7d)"): g.act(0,Move::Choose(Answer::Targets(vec![vec![Target::Object(b)]]))));
 cite!(["707.10"]; refused("a copy retains target counts and division (115.7f, 707.10)"): g.act(0,Move::Choose(Answer::Targets(vec![vec![]]))));
 assert_eq!(g.stack,before);assert_eq!(asking(&g),copy);
 g.act(0,Move::Choose(Answer::Targets(vec![vec![Target::Object(a)]]))).unwrap();pass(&mut g);cite!(["707.10c"]; assert!(g.log.iter().any(|e|matches!(e,Event::Fizzled{spell} if *spell==copy))));
}
#[test]
fn r702_56_unsupported_instances_and_unpaid_or_duplicate_announcements_are_rejected() {
 let d=def("Two","{U}","Instant","Replicate {1}\nReplicate {2}\nDraw a card.");crate::support::refused(check(&d).map_err(|e|e.0),"multiple replicate costs require separately identified announcements (702.56b)");
 let mut g=game();let a=g.put(1,1,Zone::Stack);let card=g.put(0,0,Zone::Hand);let before=g.stack.clone();let state_before=format!("{g:?}");
 cite!(["118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0,crate::support::paid("CCCCCU",Move::WithReplicate(6,Box::new(Move::Cast{card,targets:vec![vec![Target::Object(a)]]})))));assert_eq!(g.stack,before);assert!(g.players[0].hand.contains(&card));assert_eq!(format!("{g:?}"),state_before);
 let mv=Move::Cast{card,targets:vec![vec![Target::Object(a)]]};
 cite!(["601.2b"]; refused("announce replicate payments once (601.2b)"): g.act(0,crate::support::paid("CU",Move::WithReplicate(1,Box::new(Move::WithReplicate(1,Box::new(mv)))))));
 assert_eq!(format!("{g:?}"),state_before);
}
#[test]
fn r707_10_copies_keep_x_and_kicker_but_not_cast_events() {
 let spell=def("Echo","{X}{U}","Instant","Replicate {1}\nKicker {1}\nEcho deals X damage to any target. If this spell was kicked, draw a card.");check(&spell).unwrap();
 let mut g=Game::new(vec![spell,def("Filler","{0}","Artifact","")],2,20);for _ in 0..10{g.put(0,1,Zone::Library);}g.players[0].pool.colors[1]=1;g.players[0].pool.colorless=5;
 let card=g.put(0,0,Zone::Hand);let mv=Move::Cast{card,targets:vec![vec![Target::Player(1)]]};
 g.act(0,crate::support::paid("CCCCCU", Move::WithX(3,Box::new(Move::WithKicks(1,Box::new(Move::WithReplicate(1,Box::new(mv)))))))).unwrap();pass(&mut g);let copy=asking(&g);
 cite!(["707.10"]; assert_eq!(g.obj(copy).x,Some(3)));cite!(["707.10"]; assert_eq!(g.obj(copy).kicked,1));assert_eq!(g.obj(copy).owner,0);
 g.act(0,Move::Choose(Answer::No)).unwrap();pass(&mut g);pass(&mut g);cite!(["707.10"]; assert_eq!(g.players[1].life,14));assert_eq!(g.players[0].hand.len(),2);
 cite!(["707.10"]; assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Cast{..})).count(),1));
}
#[test]
fn r707_10_modal_spell_retains_modes_and_target_count() {
 let spell=def("Modal","{0}","Instant","Replicate {1}\nChoose one —\n• You gain 9 life.\n• Modal deals 2 damage to each of one or two targets.");check(&spell).unwrap();
 let mut g=Game::new(vec![spell],2,20);g.players[0].pool.colorless=1;let card=g.put(0,0,Zone::Hand);
 let mv=Move::Cast{card,targets:vec![vec![Target::Player(0),Target::Player(1)]]};g.act(0,crate::support::paid("C", Move::Modes(vec![1],Box::new(Move::WithReplicate(1,Box::new(mv)))))).unwrap();pass(&mut g);let copy=asking(&g);cite!(["707.10"]; assert_eq!(g.obj(copy).modes,vec![1]));
 cite!(["707.10"]; refused("a copy retains target counts and division (115.7f, 707.10)"): g.act(0,Move::Choose(Answer::Targets(vec![vec![Target::Player(1)]]))));
 cite!(["115.3"]; refused("a target cannot be chosen twice in one group (115.3)"): g.act(0,Move::Choose(Answer::Targets(vec![vec![Target::Player(1);2]]))));
 g.act(0,Move::Choose(Answer::No)).unwrap();pass(&mut g);pass(&mut g);cite!(["707.10"]; assert_eq!(g.players[0].life,16));assert_eq!(g.players[1].life,16);
}
#[test]
fn r707_10_ordinary_copy_instruction_uses_copy_controller_and_does_not_retrigger_replicate() {
 let mut g=game();let target=g.put(1,1,Zone::Stack);let card=g.put(0,0,Zone::Hand);cast(&mut g,card,target,1);
 let original=g.stack[1];let replicate=g.stack[2];assert!(g.counter(replicate));
 g.act(0,Move::Pass).unwrap();let fork=g.put(1,3,Zone::Hand);g.act(1,Move::Cast{card:fork,targets:vec![vec![Target::Object(original)]]}).unwrap();pass(&mut g);let copy=asking(&g);
 cite!(["707.10"]; assert_eq!(g.obj(copy).owner,1));cite!(["707.10"]; assert_eq!(g.obj(copy).controller,1));cite!(["707.10"]; assert_eq!(g.obj(copy).replicated,1));
 g.act(1,Move::Choose(Answer::No)).unwrap();cite!(["702.56a", "707.10"]; assert_eq!(g.stack.len(),3));assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Cast{..})).count(),2);
 pass(&mut g);pass(&mut g);assert!(g.stack.is_empty());assert_eq!(g.players[1].graveyard.len(),2);
}
#[test]
fn r702_56_managed_lifecycle_keeps_original_lki_until_the_trigger_finishes() {
 use game::simulation::Simulation;
 fn reply(s:&mut Simulation,mv:Move){let r=s.request().unwrap().unwrap().clone();s.respond(r.id,r.player,mv).unwrap();}
 let mut g=game();let target=g.put(1,1,Zone::Stack);let card=g.put(0,0,Zone::Hand);cast(&mut g,card,target,2);let original=g.stack[1];assert!(g.counter(original));
 let mut sim=Simulation::new(g);reply(&mut sim,Move::Pass);reply(&mut sim,Move::Pass);
 let r=sim.request().unwrap().unwrap().clone();assert!(matches!(r.question,game::legal::Asked::Retarget{..}));
 for viewer in [None,Some(0),Some(1)] {sim.snapshot(viewer).unwrap();}
 reply(&mut sim,Move::Choose(Answer::No));let next=sim.request().unwrap().unwrap().clone();assert_ne!(r.id,next.id);
 assert_eq!(sim.respond(r.id,r.player,Move::Choose(Answer::No)),Err(game::simulation::ResponseError::Stale{expected:next.id,received:r.id}));
 reply(&mut sim,Move::Choose(Answer::No));for _ in 0..4{reply(&mut sim,Move::Pass);}
 assert!(sim.game().unwrap().stack.is_empty());assert!(sim.fault().is_none());
}
#[test]
fn r707_10_copy_retains_discarded_cost_object_for_later_references() {
 let spell=def("Receipt","{0}","Instant","Replicate {1}\nAs an additional cost to cast this spell, discard a card.\nIf the discarded card wasn't a land card, draw a card.");check(&spell).unwrap();
 let mut g=Game::new(vec![spell,def("Filler","{0}","Artifact","")],2,20);for _ in 0..5{g.put(0,1,Zone::Library);}g.players[0].pool.colorless=1;
 let card=g.put(0,0,Zone::Hand);let paid=g.put(0,1,Zone::Hand);
 g.act(0,crate::support::paid("C", Move::WithCosts(vec![vec![paid]],Box::new(Move::WithReplicate(1,Box::new(Move::Cast{card,targets:vec![]})))))).unwrap();
 pass(&mut g);assert!(g.decision.is_none());assert_eq!(g.stack.len(),2);pass(&mut g);pass(&mut g);
 cite!(["707.10"]; assert_eq!(g.players[0].hand.len(),2));assert_eq!(g.players[0].graveyard.len(),2);
}
#[test]
fn r115_1_consign_union_admits_triggered_abilities_and_colorless_spells_only() {
 let defs=vec![def("Consign to Memory","{U}","Instant",TEXT),def("Blank","{0}","Artifact",""),
     def("Blue","{U}","Instant","Draw a card."),def("Watch","{0}","Enchantment","Whenever you cast a spell, you gain 1 life."),
     def("Engine","{0}","Artifact","{0}: You gain 1 life.")];for d in &defs {check(d).unwrap();}
 let mut g=Game::new(defs,2,20);let watcher=g.put(0,3,Zone::Battlefield);let engine=g.put(0,4,Zone::Battlefield);
 let blank=g.put(0,1,Zone::Hand);g.act(0,Move::Cast{card:blank,targets:vec![]}).unwrap();let triggered=*g.stack.last().unwrap();assert_eq!(g.obj(triggered).ability.unwrap().0,Some(watcher));
 let colorless=g.stack[g.stack.len()-2];
 g.act(0,Move::Activate{source:engine,ability:0,targets:vec![],paying:vec![]}).unwrap();let activated=*g.stack.last().unwrap();
 let blue=g.put(0,2,Zone::Stack);let consign=g.put(0,0,Zone::Hand);g.players[0].pool.colors[1]=1;
 for (target, accepted) in [(triggered, true), (colorless, true), (activated, false), (blue, false), (watcher, false)] {
  let result = g.clone().act(0, crate::support::paid("U", Move::Cast { card: consign, targets: vec![vec![Target::Object(target)]] }));
  cite!(["115.1a", "601.2c"]; assert_eq!(result.is_ok(), accepted, "target={target:?}: {result:?}"));
 }

}
#[test]
fn r707_10_copying_emits_copy_triggers_separately_from_cast_triggers() {
 let spell=def("Echo","{0}","Instant","Replicate {1}\nYou gain 2 life.");
 let watch=def("Watch","{0}","Enchantment","Whenever you copy a spell, you gain 3 life.");check(&watch).unwrap();
 let mut g=Game::new(vec![spell,watch],2,20);g.put(0,1,Zone::Battlefield);g.players[0].pool.colorless=1;let card=g.put(0,0,Zone::Hand);
 g.act(0,crate::support::paid("C", Move::WithReplicate(1,Box::new(Move::Cast{card,targets:vec![]})))).unwrap();pass(&mut g);
 assert_eq!(g.stack.len(),3);cite!(["707.10"]; assert_eq!(g.log.iter().filter(|e|matches!(e,Event::SpellCopied{..})).count(),1));
 pass(&mut g);pass(&mut g);pass(&mut g);cite!(["707.10"]; assert_eq!(g.players[0].life,27));
}
#[test]
fn r707_10f_copying_a_permanent_spell_resolves_as_a_token() {
 let copy=def("Copier","{0}","Instant","Copy target spell.");check(&copy).unwrap();
 let permanent=CardDef::new("Bear","{0}","Creature — Bear","",Some((2,2))).unwrap();
 let mut g=Game::new(vec![copy,permanent],2,20);let original=g.put(1,1,Zone::Stack);let card=g.put(0,0,Zone::Hand);
 g.act(0,Move::Cast{card,targets:vec![vec![Target::Object(original)]]}).unwrap();pass(&mut g);let copy=*g.stack.last().unwrap();assert!(!g.obj(copy).token());pass(&mut g);
 assert_eq!(g.battlefield.len(),1);let made=g.battlefield[0];cite!(["707.10f"]; assert!(g.obj(made).token()));assert_eq!(g.obj(made).chars.pt,Some((2,2)));assert_eq!(g.obj(made).controller,0);
}
