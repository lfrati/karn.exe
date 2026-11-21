//! Target relationships read announced groups, independent of body execution (601.2c).
use crate::support::cite;
use game::{CardDef,Game,Move,Target,Zone};
fn game(prefix:bool)->Game {
 let text=format!("{}This spell deals 2 damage to target opponent or planeswalker and 2 damage to up to one target creature that player or that planeswalker's controller controls.",if prefix {"Target player gains 1 life.\n"}else{""});
 let spell=CardDef::new("Directed","{0}","Instant",&text,None).unwrap();
 let bear=CardDef::new("Bear","{0}","Creature — Bear","",Some((3,3))).unwrap();
 let walker=CardDef::planeswalker("Walker","{0}","Legendary Planeswalker — Jace","+1: You gain 1 life.",4).unwrap();
 for d in [&spell,&bear,&walker]{game::play::check(d).unwrap();}
 Game::new(vec![spell,bear,walker],3,20)
}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
#[test]
fn r601_2c_players_and_planeswalker_controllers_restrict_the_later_target(){
 for walker in [false,true]{
  let mut g=game(false);let mine=g.put(0,1,Zone::Battlefield);let theirs=g.put(1,1,Zone::Battlefield);
  let other=g.put(2,1,Zone::Battlefield);let pw=g.put(1,2,Zone::Battlefield);g.put_counters(pw,"loyalty",4);let card=g.put(0,0,Zone::Hand);
  let first=if walker {Target::Object(pw)}else{Target::Player(1)};
  for (invalid,whose) in [(mine,"you control it"),(other,"another player controls it")]{let why=format!("an illegal target, Bear: the target is one the player it names controls, and {whose} (115.1)");cite!(["601.2c","115.1"]; case: format!("{walker} {whose}"); refused(&why): g.act(0,Move::Cast{card,targets:vec![vec![first],vec![Target::Object(invalid)]]}));}
  cite!(["601.2c"]; case: format!("{walker}"); ok: g.act(0,Move::Cast{card,targets:vec![vec![first],vec![Target::Object(theirs)]]}));
  let clone=g.clone();for mut g in [g,clone]{pass(&mut g);cite!(["120.3e"]; case: format!("{walker}"); assert_eq!(g.obj(theirs).damage,2));assert_eq!(g.obj(mine).damage,0);assert_eq!(g.obj(other).damage,0);
   if walker {assert_eq!(g.obj(pw).counters["loyalty"],2);}else{assert_eq!(g.players[1].life,18);}
  }
 }
}
#[test]
fn r113_2c_dependency_indices_survive_preceding_spell_instructions(){
 let mut g=game(true);let creature=g.put(1,1,Zone::Battlefield);let card=g.put(0,0,Zone::Hand);
 g.act(0,Move::Cast{card,targets:vec![vec![Target::Player(2)],vec![Target::Player(1)],vec![Target::Object(creature)]]}).unwrap();pass(&mut g);
 assert_eq!(g.players[2].life,21);assert_eq!(g.players[1].life,18);cite!(["120.3e"]; assert_eq!(g.obj(creature).damage,2));
}
#[test]
fn r608_2b_an_illegal_first_target_still_supplies_last_known_controller(){
 let mut g=game(false);let creature=g.put(1,1,Zone::Battlefield);let pw=g.put(1,2,Zone::Battlefield);g.put_counters(pw,"loyalty",4);let card=g.put(0,0,Zone::Hand);
 g.act(0,Move::Cast{card,targets:vec![vec![Target::Object(pw)],vec![Target::Object(creature)]]}).unwrap();
 g.move_object(pw,Zone::Graveyard);pass(&mut g);
 // as Searing Blaze's ruling (2010-03-01): either target illegal, it still deals damage to the other
 cite!(["608.2b"]; assert_eq!(g.obj(creature).damage,2,"the still-legal creature is affected even though the walker left"));
}
#[test]
fn r601_2c_an_optional_dependent_group_needs_no_creature(){
 let mut g=game(false);let card=g.put(0,0,Zone::Hand);
 cite!(["601.2c", "115.6"]; ok: g.act(0,Move::Cast{card,targets:vec![vec![Target::Player(1)],vec![]]}));pass(&mut g);assert_eq!(g.players[1].life,18);
}

#[test]
fn r603_3d_triggered_target_requests_keep_the_dependency(){
 let watcher=CardDef::new("Watcher","{0}","Creature — Goblin","When this creature enters, it deals 2 damage to target opponent or planeswalker and 2 damage to up to one target creature that player or that planeswalker's controller controls.",Some((3,3))).unwrap();
 game::play::check(&watcher).unwrap();
 let bear=CardDef::new("Bear","{0}","Creature — Bear","",Some((3,3))).unwrap();
 let mut g=Game::new(vec![watcher,bear],3,20);let one=g.put(1,1,Zone::Battlefield);let two=g.put(2,1,Zone::Battlefield);
 let card=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![]}).unwrap();pass(&mut g);
 assert!(matches!(g.decision,Some(game::state::Decision::Targets{player:0,..})));
 cite!(["603.3d", "115.1"]; refused("an illegal target, Bear: the target is one the player it names controls, and another player controls it (115.1)"): g.act(0,Move::ChooseTargets(vec![vec![Target::Player(1)],vec![Target::Object(two)]])));
 cite!(["603.3d"]; ok: g.act(0,Move::ChooseTargets(vec![vec![Target::Player(1)],vec![Target::Object(one)]])));pass(&mut g);
 assert_eq!(g.players[1].life,18);cite!(["120.3e"]; assert_eq!(g.obj(one).damage,2));assert_eq!(g.obj(two).damage,0);
}
#[test]
fn r601_2c_ambiguous_prior_target_references_are_refused(){
 let def=CardDef::new("Ambiguous","{0}","Instant","Target player gains 1 life. Target player gains 2 life. Destroy target creature that player controls.",None).unwrap();
 assert!(game::play::check(&def).unwrap_err().0.contains("ambiguous"));
}
