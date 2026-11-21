//! CR 607.2q/400.7d: entry reads the spell's actual exile payments, and graveyard
//! departure triggers observe each qualifying card independently (603.2c).
use game::{CardDef,Game,Move,ObjId,Zone};
use game::simulation::Simulation;
use crate::support::cite;
fn defs()->Vec<CardDef>{
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
 let murktide=CardDef::from_face(faces.iter().find(|f|f.name=="Murktide Regent").unwrap()).unwrap();
 let defs=vec![murktide,
 CardDef::new("Instant","{0}","Instant","You gain 1 life.",None).unwrap(),
 CardDef::new("Sorcery","{0}","Sorcery","You gain 1 life.",None).unwrap(),
 CardDef::new("Both","{0}","Instant Sorcery","You gain 1 life.",None).unwrap(),
 CardDef::new("Bear","{0}","Creature — Bear","",Some((2,2))).unwrap(),
 CardDef::new("Land","","Land","",None).unwrap(),
 CardDef::new("Hardened Scales","{G}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",None).unwrap(),
 CardDef::new("Branching Evolution","{2}{G}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on it instead.",None).unwrap(),
 ];for d in &defs{game::play::check(d).unwrap_or_else(|e|panic!("{}: {e:?}",d.name()));}defs
}
fn cast(g:&mut Game)->ObjId{let card=g.put(0,0,Zone::Hand);let exile=[1,2,3,4,5].map(|d|g.put(0,d,Zone::Graveyard));g.players[0].pool.colors[1]=2;g.act(0,crate::support::paid("UU",Move::WithDelve(5,Box::new(Move::WithCosts(vec![exile.to_vec()],Box::new(Move::Cast{card,targets:vec![]})))))).unwrap();*g.stack.last().unwrap()}
fn passes(g:&mut Game){for _ in 0..40{if g.decision.is_some()||g.stack.is_empty(){return}g.act(g.priority,Move::Pass).unwrap();}panic!("stack did not empty");}
fn murktide(g:&Game)->ObjId{*g.battlefield.iter().find(|o|g.obj(**o).card==Some(0)).unwrap()}
#[test]
fn r607_2q_paid_cards_count_once_and_only_the_linked_cost_supplies_entry(){
 let mut g=Game::new(defs(),2,20);cast(&mut g);assert_eq!(g.exile.len(),5);passes(&mut g);let m=murktide(&g);cite!(["607.2q", "400.7d"]; assert_eq!(g.power(m),Some(6)));assert!(g.has(m,"Flying"));
 g.move_object(m,Zone::Graveyard);let grave=*g.players[0].graveyard.last().unwrap();g.move_object(grave,Zone::Battlefield);cite!(["607.2q"]; assert_eq!(g.power(murktide(&g)),Some(3)));
}
#[test]
fn r607_2q_moving_paid_cards_again_does_not_erase_the_retained_payment(){
 let mut g=Game::new(defs(),2,20);cast(&mut g);let exiled=g.exile.clone();for card in exiled{g.move_object(card,Zone::Hand);}assert!(g.exile.is_empty());passes(&mut g);cite!(["607.2q", "400.7d"]; assert_eq!(g.power(murktide(&g)),Some(6)));
}
#[test]
fn r603_2_graveyard_departures_use_each_card_and_the_owners_zone(){
 let mut g=Game::new(defs(),3,20);let own=g.put(0,0,Zone::Battlefield);let other=g.put(1,0,Zone::Battlefield);
 for (owner,def,to) in [(0,1,Zone::Hand),(0,2,Zone::Exile),(0,3,Zone::Library),(0,4,Zone::Battlefield),(1,1,Zone::Exile),(2,1,Zone::Exile)]{let card=g.put(owner,def,Zone::Graveyard);g.move_object(card,to);}
 passes(&mut g); // Low-level moves wait for the next priority checkpoint.
 g.act(g.priority,Move::Pass).unwrap();
 // Player 0 orders the three qualifying departures from their own graveyard;
 // player 1 has only one occurrence. APNAP stacking still applies (603.3b).
 let Some(game::state::Decision::Order{player:0,abilities})=g.decision.clone() else{panic!("expected own graveyard trigger order: {:?}",g.decision)};
 cite!(["603.3b"]; assert_eq!(abilities.len(),3));
 assert!(abilities.iter().all(|ability|ability.source==Some(own)&&ability.controller==0));
 g.act(0,Move::Order(vec![0,1,2])).unwrap();passes(&mut g);
 cite!(["603.2c"]; assert_eq!(g.power(own),Some(6)));cite!(["603.2c"]; assert_eq!(g.power(other),Some(4)));
}
#[test]
fn r603_2_casting_source_does_not_see_its_own_delve_departures(){
 let mut g=Game::new(defs(),2,20);let first=g.put(0,0,Zone::Battlefield);cast(&mut g);
 // The existing Regent sees three qualifying delve payments. The spell being
 // cast does not; order only the existing source's occurrences before resolving.
 let Some(game::state::Decision::Order{player:0,abilities})=g.decision.clone() else{panic!("expected existing Regent trigger order: {:?}",g.decision)};
 cite!(["603.3b"]; assert_eq!(abilities.len(),3));
 assert!(abilities.iter().all(|ability|ability.source==Some(first)&&ability.controller==0));
 g.act(0,Move::Order(vec![0,1,2])).unwrap();
 passes(&mut g);let second=*g.battlefield.iter().find(|o|**o!=first&&g.obj(**o).card==Some(0)).unwrap();cite!(["603.2c"]; assert_eq!(g.power(first),Some(6)));cite!(["113.6"]; assert_eq!(g.power(second),Some(6)));
}
#[test]
fn r616_1_retained_payment_survives_managed_entry_replacement_suspension(){
 let mut g=Game::new(defs(),2,20);let scales=g.put(0,6,Zone::Battlefield);g.put(0,7,Zone::Battlefield);cast(&mut g);let mut sim=Simulation::new(g);
 for _ in 0..4{let r=sim.request().unwrap().unwrap().clone();if matches!(r.question,game::legal::Asked::Replacement{..}){break}sim.respond(r.id,r.player,Move::Pass).unwrap();}
 let r=sim.request().unwrap().unwrap().clone();let game::legal::Asked::Replacement{count,among,..}=r.question else{panic!("replacement choice")};assert_eq!(count,3);let chosen=among.iter().find(|r|r.source==scales).unwrap().clone();let mut clone=sim.clone();
 for sim in [&mut sim,&mut clone]{sim.respond(r.id,r.player,Move::Choose(game::state::Answer::Replacement(chosen.clone()))).unwrap();let _=sim.request().unwrap();cite!(["616.1", "616.1f"]; assert_eq!(sim.game().unwrap().power(murktide(sim.game().unwrap())),Some(11)));}
}
#[test]
fn r607_2_an_unbound_exiled_with_it_is_refused(){
let d=CardDef::new("Unbound","{0}","Sorcery","Delve\nYou gain 1 life for each card exiled with it.",None).unwrap();crate::support::refused(game::play::check(&d).map_err(|e|e.0),"input reference: nothing before it supplies the required participant/cardinality");}
#[test]
fn r707_10_spell_copy_retains_original_exile_payment_objects_without_paying_again(){
 let mut ds=defs();ds.push(CardDef::new("Copier","{0}","Instant","Copy target spell.",None).unwrap());game::play::check(ds.last().unwrap()).unwrap();
 let mut g=Game::new(ds,2,20);let original=cast(&mut g);let copy=g.put(0,8,Zone::Hand);
 g.act(0,Move::Cast{card:copy,targets:vec![vec![game::Target::Object(original)]]}).unwrap();passes(&mut g);
 let dragons=g.battlefield.iter().copied().filter(|o|g.obj(*o).chars.name=="Murktide Regent").collect::<Vec<_>>();
 cite!(["707.10f"]; assert_eq!(dragons.len(),2));cite!(["707.10"]; assert_eq!(g.exile.len(),5));
 for dragon in dragons{cite!(["707.10", "607.2q"]; assert_eq!(g.power(dragon),Some(6)));}
}
