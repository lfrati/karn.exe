//! A turn's loss is a bounded per-player total, independent of later gains and event retirement.
use crate::support::{cite, paid};
use game::{CardDef,Game,Move,Zone};
use game::legal::Asked;
use game::simulation::Simulation;
use game::state::Answer;
use std::sync::OnceLock;
fn game()->Game{
 static DS:OnceLock<Vec<CardDef>>=OnceLock::new();
 let ds=DS.get_or_init(||{
  let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let catalog=CardDef::catalog(&faces);
  let ds=vec![CardDef::from_faces(catalog.get("Y'shtola, Night's Blessed").unwrap()).unwrap(),
   CardDef::from_faces(catalog.get("Bloodchief Ascension").unwrap()).unwrap(),
   CardDef::new("Island","","Basic Land — Island","",None).unwrap(),
   CardDef::new("Vessel","{3}","Artifact","",None).unwrap(),
   CardDef::from_faces(catalog.get("Delney, Streetwise Lookout").unwrap()).unwrap()];
  for d in &ds{game::play::check(d).unwrap_or_else(|e|panic!("{}: {e:?}",d.name()));}ds
 });Game::new(ds.clone(),3,20)
}
fn pass(g:&mut Game){for _ in 0..3{g.act(g.priority,Move::Pass).unwrap();}}
fn end(g:&mut Game){for _ in 0..80{if g.part==ir::Part::End{return;}let(p,q)=g.asked().unwrap();g.act(p,match q{Asked::Priority=>Move::Pass,Asked::Attackers{..}=>Move::Attack(vec![]),other=>panic!("{other:?}")}).unwrap();}panic!("didn't reach end step");}
#[test]
fn r119_3_yshtola_counts_each_players_loss_separately_and_does_not_subtract_gains(){
 for (mine,a,b,draw) in [(4,0,0,true),(0,4,0,true),(0,2,2,false),(0,3,0,false)]{
  let mut g=game();g.put(0,0,Zone::Battlefield);
  g.put(0,2,Zone::Library); // the end-step trigger may draw one
  g.put(1,2,Zone::Library); // the next turn has one normal draw
  if mine>0{assert!(g.pay_life(0,mine).unwrap());}g.lose_life(1,a).unwrap();g.lose_life(2,b).unwrap();g.gain_life(1,10).unwrap();
  g.retire_events(g.log.end());g.collect_departed();end(&mut g);
  cite!(["603.4"]; case: format!("{mine},{a},{b}"); assert_eq!(g.stack.len(),usize::from(draw)));if draw{pass(&mut g);}assert_eq!(g.players[0].hand.len(),usize::from(draw));
  // Next turn has a fresh window. The ordinary draw belongs to seat 1, not the watcher.
  pass(&mut g);end(&mut g);cite!(["603.4"]; case: format!("{mine},{a},{b}"); assert!(g.stack.is_empty()));assert_eq!(g.players[0].hand.len(),usize::from(draw));
 }
}
#[test]
fn r603_2_yshtola_cast_trigger_damages_each_opponent_and_lifegain_survives_a_clone(){
 let mut g=game();g.put(0,0,Zone::Battlefield);let card=g.put(0,3,Zone::Hand);g.players[0].pool.colorless=3;
 g.act(0,paid("CCC",Move::Cast{card,targets:vec![]})).unwrap();assert_eq!(g.stack.len(),2);
 let mut sim=Simulation::new(g);let mut copy=sim.clone();
 for s in [&mut sim,&mut copy]{for _ in 0..6{let r=s.request().unwrap().unwrap().clone();s.respond(r.id,r.player,Move::Pass).unwrap();}
  let g=s.game().unwrap();cite!(["120.3a", "119.3"]; assert_eq!(g.players.iter().map(|p|p.life).collect::<Vec<_>>(),vec![22,18,18]));assert!(g.stack.is_empty());assert_eq!(g.battlefield.len(),2);}
}
#[test]
fn r603_4_bloodchief_checks_opponent_loss_and_can_decline_the_quest_counter(){
 for (player,yes,expected) in [(0,true,0),(1,false,0),(2,true,1)]{
  let mut g=game();let asc=g.put(0,1,Zone::Battlefield);g.lose_life(player,2).unwrap();end(&mut g);
  if player!=0{pass(&mut g);assert!(matches!(g.asked(),Some((0,Asked::May))));g.act(0,Move::Choose(if yes{Answer::Yes}else{Answer::No})).unwrap();}
  cite!(["603.4"]; case: format!("{player},{yes}"); assert_eq!(g.obj(asc).counters.get("quest").copied().unwrap_or(0),expected));
 }
}
#[test]
fn r603_4_bloodchief_graveyard_trigger_checks_threshold_again_and_retains_the_player(){
 for (remove,yes,life) in [(false,true,18),(false,false,20),(true,true,20)]{
  let mut g=game();let asc=g.put(0,1,Zone::Battlefield);g.put_counters(asc,"quest",3);
  let card=g.put(2,2,Zone::Hand);g.move_object(card,Zone::Graveyard);g.act(0,Move::Pass).unwrap();assert_eq!(g.stack.len(),1);
  if remove{g.remove_counters(asc,"quest",1);}
  for _ in 0..4{if g.stack.is_empty() || g.decision.is_some(){break;}g.act(g.priority,Move::Pass).unwrap();}
  if !remove{assert!(matches!(g.asked(),Some((0,Asked::May))));g.act(0,Move::Choose(if yes{Answer::Yes}else{Answer::No})).unwrap();}
  cite!(["603.4"]; case: format!("{remove},{yes}"); assert_eq!(g.players[2].life,life));assert_eq!(g.players[1].life,20);assert_eq!(g.players[0].life,if life==18{22}else{20});
 }
}
#[test]
fn r603_6c_from_anywhere_uses_arrival_state_and_does_not_watch_its_own_departure(){
 let mut g=game();let asc=g.put(0,1,Zone::Battlefield);g.put_counters(asc,"quest",3);
 let land=g.put(1,2,Zone::Battlefield);g.move_object(land,Zone::Graveyard);
 g.act(0,Move::Pass).unwrap();cite!(["603.6c"]; assert_eq!(g.stack.len(),1,"only the arrival fact matches, not the departure fact too"));
 for _ in 0..4{if g.decision.is_some(){break;}g.act(g.priority,Move::Pass).unwrap();}
 g.act(0,Move::Choose(Answer::Yes)).unwrap();assert_eq!(g.players[1].life,18);
 g.move_object(asc,Zone::Graveyard);let other=g.put(2,2,Zone::Hand);g.move_object(other,Zone::Graveyard);
 g.act(g.priority,Move::Pass).unwrap();cite!(["603.6c", "603.10"]; assert!(g.stack.is_empty(),"a former battlefield watcher is absent in the arrival view"));
}


// Rules: 603.2d, 603.3b, 603.4, 405.2, 608.1
// Given: Delney doubles 2-power Y'shtola's end-step trigger; an opponent lost 4 life,
// so Bloodchief also triggers once. No effect changes power or the life-loss history.
// When: the controller selects Y'shtola, Bloodchief, Y'shtola in insertion order.
// Expect: the top draw resolves first, then the optional quest counter, then the other draw.
// Why: 603.3b lets the controller order these simultaneous triggers, and 405.2 puts
// each successive ability on top. The offered enumeration itself has no rules-mandated order.
#[test]
fn r603_3b_delney_doubled_yshtola_and_bloodchief_follow_the_offered_choice_indices() {
    // Either initial object enumeration must permit the same semantic player choice.
    for ascension_first in [false, true] {
        let mut g = game();
        let (yshtola, ascension) = if ascension_first {
            let ascension = g.put(0, 1, Zone::Battlefield);
            (g.put(0, 0, Zone::Battlefield), ascension)
        } else {
            let yshtola = g.put(0, 0, Zone::Battlefield);
            (yshtola, g.put(0, 1, Zone::Battlefield))
        };
        g.put(0, 4, Zone::Battlefield);
        g.put(0, 2, Zone::Library);
        g.put(0, 2, Zone::Library);
        g.lose_life(1, 4).unwrap();
        end(&mut g);
        let Some((player, Asked::Order(abilities))) = g.asked() else {
            panic!("expected the controller's trigger order question");
        };
        cite!(["603.2d", "603.3b", "603.4"]; assert_eq!(player, 0));
        let draws: Vec<_> = abilities.iter().enumerate()
            .filter_map(|(i, (source, _))| (*source == Some(yshtola)).then_some(i)).collect();
        let quests: Vec<_> = abilities.iter().enumerate()
            .filter_map(|(i, (source, _))| (*source == Some(ascension)).then_some(i)).collect();
        cite!(["603.2d", "603.4"]; assert_eq!((abilities.len(), draws.len(), quests.len()), (3, 2, 1)));
        cite!(["603.3b"]; ok: g.act(player, Move::Order(vec![draws[0], quests[0], draws[1]])));
        let sources: Vec<_> = g.stack.iter().map(|id| g.obj(*id).ability.unwrap().0).collect();
        cite!(["603.3b", "405.2"]; assert_eq!(sources, vec![Some(yshtola), Some(ascension), Some(yshtola)]));
        pass(&mut g);
        cite!(["608.1", "603.4"]; assert_eq!(g.players[0].hand.len(), 1));
        cite!(["608.1"]; assert_eq!(g.obj(ascension).counters.get("quest"), None));
        pass(&mut g);
        cite!(["608.1", "603.4"]; assert!(matches!(g.asked(), Some((0, Asked::May)))));
        g.act(0, Move::Choose(Answer::Yes)).unwrap();
        cite!(["608.1", "603.4"]; assert_eq!(g.obj(ascension).counters.get("quest"), Some(&1)));
        cite!(["608.1"]; assert_eq!(g.players[0].hand.len(), 1));
        pass(&mut g);
        cite!(["608.1", "603.4"]; assert_eq!(g.players[0].hand.len(), 2));
        cite!(["608.1"]; assert!(g.stack.is_empty()));
    }
}
