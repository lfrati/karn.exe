//! CR 702.29e: typecycling is an ordinary hand activation, discarding as its cost
//! and using the shared private search/reveal/shuffle program on resolution.
use crate::support::cite;
use game::{CardDef,Game,Move,ObjId,Zone,Event};
use game::legal::Asked;
use game::simulation::Simulation;
fn defs()->Vec<CardDef>{
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
 let lorien=CardDef::from_face(faces.iter().find(|f|f.name=="Lórien Revealed").unwrap()).unwrap();
 let d=vec![lorien,CardDef::new("Island","","Basic Land — Island","",None).unwrap(),
 CardDef::new("Dual","","Land — Island Swamp","",None).unwrap(),
 CardDef::new("Forest","","Basic Land — Forest","",None).unwrap(),
 CardDef::new("Cycler","{0}","Creature — Bear","Cycling {1}",Some((2,2))).unwrap(),
 CardDef::new("Basic cycler","{0}","Creature — Bear","Basic landcycling {1}",Some((2,2))).unwrap(),
 CardDef::new("Watcher","{0}","Enchantment","Whenever you cast a spell, you gain 5 life.\nWhenever you discard a card, you gain 2 life.",None).unwrap(),
 ];for d in &d{game::play::check(d).unwrap_or_else(|e|panic!("{}: {e:?}",d.name()));}d
}
fn respond(s:&mut Simulation,m:Move){let r=s.request().unwrap().unwrap().clone();s.respond(r.id,r.player,m).unwrap();}
fn passes(s:&mut Simulation){for _ in 0..s.game().unwrap().players.len(){respond(s,Move::Pass);}}
fn ability(g:&Game,card:ObjId)->usize{g.abilities(card).iter().position(|a|matches!(&**a,ir::Ability::Activated{..})).unwrap()}
fn activate(card:ObjId,ability:usize)->Move{Move::Activate{source:card,ability,targets:vec![],paying:vec![]}}
/// The answer, after checking that it changed nothing.
#[track_caller]
fn attempt(s:&mut Simulation,m:Move)->Result<(),game::simulation::ResponseError>{let r=s.request().unwrap().unwrap().clone();let before=format!("{:?}",s.game().unwrap());let out=s.respond(r.id,r.player,m).map(|_|());assert_eq!(format!("{:?}",s.game().unwrap()),before);out}
/// The game refuses this answer for `why` and nothing changes.
#[track_caller]
fn refuse(s:&mut Simulation,m:Move,why:&str){crate::support::refused(attempt(s,m),why);}
#[test]
fn r702_29e_islandcycling_pays_discard_then_privately_finds_basic_or_nonbasic_islands(){
 for found in [None,Some(0),Some(1)]{let mut g=Game::new(defs(),3,20);let card=g.put(0,0,Zone::Hand);let index=ability(&g,card);let islands=[g.put(0,1,Zone::Library),g.put(0,2,Zone::Library)];let forest=g.put(0,3,Zone::Library);g.players[0].pool.colorless=1;let mut sim=Simulation::new(g);
  respond(&mut sim,crate::support::paid("C",activate(card,index)));assert_eq!(sim.game().unwrap().players[0].graveyard.len(),1);assert!(sim.game().unwrap().players[0].hand.is_empty());assert_eq!(sim.game().unwrap().players[0].pool.total(),0);passes(&mut sim);
  let r=sim.request().unwrap().unwrap().clone();let Asked::Search{cards,among,min,max}=&r.question else{panic!("private search")};assert_eq!((*min,*max),(0,1));cite!(["702.29e"]; assert_eq!(among,&islands));assert_eq!(cards.len(),3);for c in cards{assert!(sim.game().unwrap().sees(0,*c));assert!(!sim.game().unwrap().sees(1,*c));assert!(!sim.game().unwrap().sees(2,*c));}
  // a Forest is no Island; one card is within the count
  cite!(["701.23a"]; refused("a chosen card is not one the search can find, in the searched zone and matching its description (701.23a): choose between 0 and 1 distinct matching objects"): attempt(&mut sim,Move::Choose(game::state::Answer::Objects(vec![forest]))));let mut cloned=sim.clone();for sim in [&mut sim,&mut cloned]{respond(sim,Move::Choose(game::state::Answer::Objects(found.map(|i|vec![islands[i]]).unwrap_or_default())));cite!(["702.29e", "701.23b"]; assert_eq!(sim.game().unwrap().players[0].hand.len(),usize::from(found.is_some())));assert_eq!(sim.game().unwrap().players[0].library.len(),3-usize::from(found.is_some()));assert!(!sim.game().unwrap().sees(0,forest));}
 }
}
#[test]
fn r702_29a_cycling_is_owner_hand_only_and_not_subject_to_sorcery_timing(){
 // cycling functions only in the hand it discards from (113.6m, 702.29a), wherever else it is
 let why="that ability functions only in the zone it moves its card out of (113.6m)";
 for zone in [Zone::Graveyard,Zone::Battlefield,Zone::Exile,Zone::Library]{let mut g=Game::new(defs(),2,20);let card=g.put(0,0,zone);let index=ability(&g,card);g.players[0].pool.colorless=1;let mut sim=Simulation::new(g);refuse(&mut sim,activate(card,index),why);}
 let mut g=Game::new(defs(),2,20);g.active=1;let other=g.put(1,0,Zone::Hand);let card=g.put(0,0,Zone::Hand);let index=ability(&g,card);g.players[0].pool.colorless=1;g.put(0,1,Zone::Library);let mut sim=Simulation::new(g);refuse(&mut sim,activate(other,index),"an ability of a card outside the battlefield is activated by its owner (108.4a, 602.2)");respond(&mut sim,crate::support::paid("C",activate(card,index)));passes(&mut sim);assert!(matches!(sim.request().unwrap().unwrap().question,Asked::Search{..}));
}
#[test]
fn r702_29e_missing_payment_leaves_the_card_in_hand(){let mut g=Game::new(defs(),2,20);let card=g.put(0,0,Zone::Hand);let index=ability(&g,card);let mut sim=Simulation::new(g);refuse(&mut sim,activate(card,index),"your mana pool cannot pay that cost (118.3a): {1}");assert_eq!(sim.game().unwrap().obj(card).zone, Zone::Hand);}
#[test]
fn r702_29f_typecycling_triggers_discard_not_cast_and_keeps_the_cycling_characteristic(){
 let mut g=Game::new(defs(),2,20);g.put(0,6,Zone::Battlefield);let card=g.put(0,0,Zone::Hand);cite!(["702.29f"]; assert!(g.has(card,"Cycling")));let index=ability(&g,card);g.players[0].pool.colorless=1;g.act(0,crate::support::paid("C",activate(card,index))).unwrap();assert!(!g.log.iter().any(|e|matches!(e,Event::Cast{..})));cite!(["702.29e"]; assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Discarded{..})).count(),1));g.act(0,Move::Pass).unwrap();g.act(1,Move::Pass).unwrap();cite!(["702.29e"]; assert_eq!(g.players[0].life,22));
}
#[test]
fn r702_29a_ordinary_cycling_draws_one_and_basic_typecycling_intersects_its_types(){
 for def in [4,5]{let mut g=Game::new(defs(),2,20);let card=g.put(0,def,Zone::Hand);let index=ability(&g,card);let basic=g.put(0,1,Zone::Library);let nonbasic=g.put(0,2,Zone::Library);g.players[0].pool.colorless=1;let mut sim=Simulation::new(g);respond(&mut sim,crate::support::paid("C",activate(card,index)));passes(&mut sim);if def==4{cite!(["702.29a"]; assert_eq!(sim.game().unwrap().players[0].hand.len(),1));}else{let Asked::Search{among,..}=&sim.request().unwrap().unwrap().question else{panic!("search")};cite!(["702.29e"]; assert_eq!(among,&vec![basic]));assert!(!among.contains(&nonbasic));}}
}
#[test]
fn r601_lorien_casts_normally_and_draws_three_without_a_cycling_search(){let mut g=Game::new(defs(),2,20);let card=g.put(0,0,Zone::Hand);for _ in 0..3{g.put(0,1,Zone::Library);}g.players[0].pool.colors[1]=5;let mut sim=Simulation::new(g);respond(&mut sim,crate::support::paid("UUUUU",Move::Cast{card,targets:vec![]}));passes(&mut sim);assert_eq!(sim.game().unwrap().players[0].hand.len(),3);assert!(sim.game().unwrap().players[0].library.is_empty());assert!(matches!(sim.request().unwrap().unwrap().question,Asked::Priority));}
#[test]
fn r702_29e_countering_the_activation_does_not_refund_the_discard(){let mut g=Game::new(defs(),2,20);let card=g.put(0,0,Zone::Hand);let index=ability(&g,card);g.put(0,1,Zone::Library);g.players[0].pool.colorless=1;g.act(0,crate::support::paid("C",activate(card,index))).unwrap();assert!(g.counter(g.stack[0]));assert!(g.stack.is_empty());assert_eq!(g.players[0].graveyard.len(),1);cite!(["701.6a"]; assert_eq!(g.players[0].library.len(),1));assert_eq!(g.players[0].pool.total(),0);assert!(g.decision.is_none());}
#[test]
fn r702_29_unimplemented_cycling_trigger_and_granted_cost_variants_remain_refused(){
 for (text,reason) in [("Cycling {1}\nWhen you cycle this card, you gain 2 life.","trigger: Cycle"),("Creature cards in your hand have cycling {1}.","keyword: Cycling")] {
 let d=CardDef::new("Unsupported","{0}","Creature — Bear",text,Some((2,2))).unwrap();
 crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
 }
}
#[test]
fn r602_2a_paused_hand_activation_reveals_its_source_and_reversal_ends_the_reveal(){
 let mut cards=defs();
 cards.extend([
  CardDef::new("Mana creature","{0}","Creature — Elf","{T}: You gain 2 life. Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.",Some((1,1))).unwrap(),
  CardDef::new("Scales","{G}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",None).unwrap(),
  CardDef::new("Evolution","{2}{G}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.",None).unwrap(),
 ]);
 let mut g=Game::new(cards,2,20);let card=g.put(0,0,Zone::Hand);let hidden=g.put(0,4,Zone::Hand);let index=ability(&g,card);let source=g.put(0,7,Zone::Battlefield);g.put(0,8,Zone::Battlefield);let evolution=g.put(0,9,Zone::Battlefield);g.put(0,1,Zone::Library);
 let mut sim=Simulation::new(g);assert!(!sim.snapshot(Some(1)).unwrap().objects.iter().any(|o|o.id==card));
 let step=game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source,ability:0,paying:vec![],adding:vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Green);3]]};
 respond(&mut sim,Move::WithMana(vec![step.clone(),step],Box::new(activate(card,index))));
 assert_eq!(sim.game().unwrap().obj(card).zone,Zone::Hand);
 let visible=sim.snapshot(Some(1)).unwrap();assert!(visible.objects.iter().any(|o|o.id==card));assert!(!visible.objects.iter().any(|o|o.id==hidden));
 let Some(game::state::Decision::Choose{choice:game::state::Choice::Replacement{among,..},..})=&sim.game().unwrap().decision else{panic!("replacement")};let effect=among.iter().find(|e|e.source==evolution).unwrap().clone();respond(&mut sim,Move::Choose(game::state::Answer::Replacement(effect)));
 assert!(matches!(sim.game().unwrap().decision,Some(game::state::Decision::Payment{..})));
 let mut reversed=sim.clone();respond(&mut reversed,Move::ReversePayment);
 assert_eq!(reversed.game().unwrap().obj(card).zone,Zone::Hand);assert!(reversed.game().unwrap().stack.is_empty());assert_eq!(reversed.game().unwrap().players[0].pool.total(),3);assert!(reversed.game().unwrap().obj(source).tapped);assert!(!reversed.snapshot(Some(1)).unwrap().objects.iter().any(|o|o.id==card));
 respond(&mut sim,Move::FinishPayment{plan:vec![],paying:vec![],with:Some(crate::support::mana_pool("G"))});assert_eq!(sim.game().unwrap().players[0].pool.total(),2);cite!(["602.2a"]; assert_eq!(sim.game().unwrap().players[0].graveyard.len(),1));assert_eq!(sim.game().unwrap().stack.len(),1);passes(&mut sim);assert!(matches!(sim.request().unwrap().unwrap().question,Asked::Search{..}));
}

/// Zenith Flare: "Zenith Flare deals X damage to any target and you gain X life, where X is the
/// number of cards with a cycling ability in your graveyard." Typecycling abilities are cycling
/// abilities (702.29f): the swampcycler counts, the Bears do not.
#[test]
fn r702_29f_zenith_flare_counts_typecycling_cards_as_cards_with_a_cycling_ability() {
    use crate::support::{Board, mana_pool, player};
    let mut t = Board::new(2).graveyard(0, &["Drifting Meadow", "Twisted Abomination", "Grizzly Bears"]).hand(0, &["Zenith Flare"]).build();
    t.mana(0, "{2}{R}{W}").cast(0, "Zenith Flare", vec![player(1)], mana_pool("CCRW")).unwrap().resolve();
    cite!(["702.29f", "107.3a"]; assert_eq!((t.g.players[1].life, t.g.players[0].life), (18, 22)));
}
