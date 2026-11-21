//! Reflexive triggers (603.12) on pinned admitted witnesses:
//! per-occurrence event binding, the source after its own sacrifice, and a duration keyed to a
//! source that left before the reflexive occurrence resolved.
use crate::support::{cite, mana_pool, paid};
use game::{CardDef,Game,Move,Target,Zone};
use game::state::{Answer,Decision};
fn card(name:&str,kind:&str,text:&str,pt:Option<(i32,i32)>)->CardDef{CardDef::new(name,"{0}",kind,text,pt).unwrap()}
fn pinned(name:&str)->CardDef{
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let catalog=CardDef::catalog(&faces);
 let d=CardDef::from_faces(catalog.get(name).unwrap()).unwrap();game::play::check(&d).unwrap_or_else(|e|panic!("{name}: {}",e.0));d
}
fn pass(g:&mut Game){for _ in 0..3{g.act(g.priority,Move::Pass).unwrap();}}
fn yes(g:&mut Game){g.act(0,Move::Choose(Answer::Yes)).unwrap();}
fn targets(g:&mut Game,ts:Vec<Vec<Target>>){g.act(0,Move::ChooseTargets(ts)).unwrap();}

/// Anointer of Valor: each reflexive occurrence keeps the attacking creature of its own parent
/// event ("that creature"), not the latest or a shared binding.
#[test]
fn anointer_two_attackers_each_occurrence_counts_its_own_creature(){
 let mut g=Game::new(vec![pinned("Anointer of Valor"),card("Body","Creature","",Some((1,1)))],3,20);
 g.put(0,0,Zone::Battlefield);let a=g.put(0,1,Zone::Battlefield);let b=g.put(0,1,Zone::Battlefield);
 pass(&mut g);pass(&mut g);g.act(0,Move::Attack(vec![(a,Target::Player(1)),(b,Target::Player(1))])).unwrap();
 if matches!(g.decision,Some(Decision::Order{..})){g.act(0,Move::Order(vec![0,1])).unwrap();}
 assert_eq!(g.stack.len(),2);g.players[0].pool.colorless=6;
 // the top parent resolves and is paid; its reflexive occurrence goes above the other parent
 pass(&mut g);g.act(0,Move::Choose(Answer::Pay {plan:vec![],with:Some(mana_pool("CCC")),tapped:vec![]})).unwrap();cite!(["603.12"]; assert_eq!(g.stack.len(),2));
 pass(&mut g);let counters=|g:&Game,o|g.obj(o).counters.get("+1/+1").copied().unwrap_or(0);
 assert_eq!(counters(&g,a)+counters(&g,b),1);
 pass(&mut g);g.act(0,Move::Choose(Answer::Pay {plan:vec![],with:Some(mana_pool("CCC")),tapped:vec![]})).unwrap();pass(&mut g);
 cite!(["603.12"]; assert_eq!((counters(&g,a),counters(&g,b)),(1,1)));
}

/// Eden: "another target permanent card" excludes Eden itself after it was sacrificed by the
/// parent: the graveyard card is a new object (400.7) but the same card as the source.
#[test]
fn eden_cannot_return_itself_after_its_sacrifice(){
 let mut g=Game::new(vec![pinned("Eden, Seat of the Sanctum"),card("Body","Creature","",Some((1,1)))],3,20);
 let eden=g.put(0,0,Zone::Battlefield);let other=g.put(0,1,Zone::Graveyard);let a=g.put(0,1,Zone::Library);let b=g.put(0,1,Zone::Library);
 g.players[0].pool.colorless=5;g.act(0,paid("CCCCC",Move::Activate{source:eden,ability:1,targets:vec![],paying:vec![]})).unwrap();
 pass(&mut g);
 cite!(["404.3"]; ok: g.act(0,Move::Choose(Answer::Objects(vec![a,b]))));
 yes(&mut g);
 if matches!(g.decision,Some(Decision::Choose{..})){g.act(0,Move::Choose(Answer::Objects(vec![eden]))).unwrap();}
 cite!(["603.12"]; assert!(matches!(g.decision,Some(Decision::Targets{..})),"{:?}",g.decision));
 let grave=*g.players[0].graveyard.iter().find(|o|g.obj(**o).card==Some(0)).expect("Eden in graveyard");
 // Uncited (card text): "another" exists to exclude the source's own card, though it is a new object (400.7); Myr Retriever f56ee1e67b25be80 and Workshop Assistant 65fb75538b4783cb allow only another card that arrived with it
 crate::support::refused(g.clone().act(0,Move::ChooseTargets(vec![vec![Target::Object(grave)]])), "an illegal target, Eden, Seat of the Sanctum: it is the object whose ability this is, and the target says \"another\" (115.1)");
 targets(&mut g,vec![vec![Target::Object(other)]]);pass(&mut g);
 assert!(g.players[0].hand.iter().any(|o|g.obj(*o).card==Some(1)));
}

/// Shire Shirriff ruling f4756aca43800f6b: if the source left before the reflexive ability
/// resolves, the target is not exiled; while it stays, the exile lasts until it leaves (610.3).
#[test]
fn shirriff_reflexive_duration_uses_the_source_left_before_resolution(){
 for leaves in [false,true]{
  let maker=card("Maker","Sorcery","Create a 1/1 white Soldier creature token.",None);
  let kill=card("Kill","Instant","Destroy target creature.",None);
  let mut g=Game::new(vec![pinned("Shire Shirriff"),maker,card("Body","Creature","",Some((1,1))),kill],3,20);
  let destroy=|g:&mut Game,o|{let k=g.put(0,3,Zone::Hand);g.act(0,Move::Cast{card:k,targets:vec![vec![Target::Object(o)]]}).unwrap();pass(g);};
  let victim=g.put(1,2,Zone::Battlefield);
  let m=g.put(0,1,Zone::Hand);g.act(0,Move::Cast{card:m,targets:vec![]}).unwrap();pass(&mut g);
  let token=g.battlefield.iter().copied().find(|o|*o!=victim).unwrap();
  g.players[0].pool.colors[0]=2;let s=g.put(0,0,Zone::Hand);g.act(0,paid("WW",Move::Cast{card:s,targets:vec![]})).unwrap();pass(&mut g);
  let shirriff=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(0)).unwrap();
  assert_eq!(g.stack.len(),1);pass(&mut g);yes(&mut g);
  if matches!(g.decision,Some(Decision::Choose{..})){g.act(0,Move::Choose(Answer::Objects(vec![token]))).unwrap();}
  targets(&mut g,vec![vec![Target::Object(victim)]]);cite!(["603.12"]; case: format!("leaves={leaves}"); assert_eq!(g.stack.len(),1));
  if leaves{destroy(&mut g,shirriff);assert!(!g.battlefield.contains(&shirriff));assert_eq!(g.stack.len(),1);}
  pass(&mut g);
  cite!(["610.3b"]; case: format!("leaves={leaves}"); assert_eq!(g.battlefield.contains(&victim),leaves,"leaves={leaves}"));
  if !leaves{destroy(&mut g,shirriff);
   cite!(["610.3"]; assert!(g.battlefield.iter().any(|o|g.obj(*o).card==Some(2)),"victim returns when Shirriff leaves"));}
 }
}
