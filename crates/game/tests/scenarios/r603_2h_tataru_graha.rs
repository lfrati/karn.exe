//! Full definitions compose trigger/action limits with references and produced tokens.
use game::{CardDef, Game, Move, Zone, Target};
use game::state::{Answer, Choice, Decision};
use crate::support::{cite, paid};
fn setup(name: &str) -> Game {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let defs=vec![CardDef::from_face(faces.iter().find(|f|f.face_name==name).unwrap()).unwrap(),
        CardDef::new("Forest","","Basic Land — Forest","",None).unwrap(),
        CardDef::new("Draw","{0}","Instant","Target player draws two cards.",None).unwrap(),
        CardDef::new("Spell","{3}","Instant","You gain 1 life.",None).unwrap(),
        CardDef::new("Empty","{0}","Instant","You gain 0 life.",None).unwrap(),
        CardDef::new("Variable","{X}","Instant","You gain 0 life.",None).unwrap()];
    for d in &defs{game::play::check(d).unwrap_or_else(|e|panic!("{}: {e:?}",d.name()));}
    let mut g=Game::new(defs,3,20);g.put(0,0,Zone::Battlefield);
    for p in 0..3{for _ in 0..12{g.put(p,1,Zone::Library);}}
    g
}
fn pass(g:&mut Game){for _ in 0..3{g.act(g.priority,Move::Pass).unwrap();}}
fn cast(g:&mut Game,def:usize,targets:Vec<Vec<Target>>,spending:&str){
    while g.priority!=0{g.act(g.priority,Move::Pass).unwrap();}
    if def==3 {for _ in 0..3{g.add_mana(0,ir::mana::ManaSymbol::Color(ir::Color::Green));}}
    let o=g.put(0,def,Zone::Hand);g.act(0,paid(spending,Move::Cast{card:o,targets})).unwrap();
}
#[test]
fn tataru_off_turn_draws_make_one_tapped_treasure_and_own_turn_draws_do_not_use_limit(){
    let mut g=setup("Tataru Taru");
    cast(&mut g,2,vec![vec![Target::Player(1)]], "");pass(&mut g);
    assert_eq!(g.stack.len(),1,"two draws trigger once");pass(&mut g);
    let treasures:Vec<_>=g.battlefield.iter().filter(|o|g.obj(**o).token() && g.obj(**o).chars.name.contains("Treasure")).copied().collect();
    // Uncited (card text): "create a tapped Treasure token" (the tapped check).
    assert_eq!(treasures.len(),1,"{:?}",g.log);assert!(g.obj(treasures[0]).tapped);
    cast(&mut g,2,vec![vec![Target::Player(2)]], "");pass(&mut g);assert!(g.stack.is_empty());
}
#[test]
fn tataru_entry_controller_draw_precedes_target_opponents_optional_draw(){
    let mut g=setup("Tataru Taru");let source=g.battlefield[0];g.move_object(source,Zone::Hand);
    let card=g.players[0].hand[0];g.move_object(card,Zone::Battlefield);
    g.act(g.priority,Move::Pass).unwrap();
    assert!(matches!(g.decision,Some(Decision::Targets{player:0,..})));
    g.act(0,Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();pass(&mut g);
    assert_eq!(g.players[0].hand.len(),1);
    cite!(["608.2c"]; assert!(matches!(g.decision,Some(Decision::Choose{player:1,choice:Choice::May}))));
    let mut clone=g.clone();
    for game in [&mut g,&mut clone]{game.act(1,Move::Choose(Answer::No)).unwrap();assert_eq!(game.players[0].hand.len(),1);assert!(game.players[1].hand.is_empty());}
}
#[test]
fn graha_life_payment_keeps_spell_mana_value_for_the_created_hero_counters(){
    let mut g=setup("G'raha Tia, Scion Reborn");
    for _ in 0..3{g.put(0,1,Zone::Battlefield);}cast(&mut g,3,vec![], "GGG");pass(&mut g);
    assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Pay{life:3,..}})));
    let mut clone=g.clone();for game in [&mut g,&mut clone]{
        game.act(0,Move::Choose(Answer::Pay { plan: vec![], with: None, tapped: vec![] })).unwrap();assert_eq!(game.players[0].life,17);
        let hero=*game.battlefield.iter().find(|o|game.obj(**o).token()).unwrap();
        assert_eq!(game.obj(hero).counters.get("+1/+1"),Some(&3));assert_eq!(game.stack.len(),1);
        cast(game,4,vec![], "");cite!(["603.2h"]; assert_eq!(game.stack.len(),2,"paid ability no longer triggers"));
    }
}
#[test]
fn graha_decline_keeps_allowance_and_zero_life_payment_is_still_taken(){
    let mut g=setup("G'raha Tia, Scion Reborn");cast(&mut g,4,vec![], "");pass(&mut g);
    g.act(0,Move::Choose(Answer::No)).unwrap();pass(&mut g);
    cast(&mut g,4,vec![], "");pass(&mut g);g.act(0,Move::Choose(Answer::Pay { plan: vec![], with: None, tapped: vec![] })).unwrap();
    assert_eq!(g.players[0].life,20);let hero=*g.battlefield.iter().find(|o|g.obj(**o).token()).unwrap();
    assert!(g.obj(hero).counters.is_empty());cast(&mut g,4,vec![], "");cite!(["603.2h", "118.3b"]; assert_eq!(g.stack.len(),2));
}

#[test]
fn tataru_own_turn_draws_leave_off_turn_allowance_available(){
    let mut g=setup("Tataru Taru");g.active=1;
    cast(&mut g,2,vec![vec![Target::Player(1)]], "");pass(&mut g);cite!(["603.4"]; assert!(g.stack.is_empty()));
    cast(&mut g,2,vec![vec![Target::Player(2)]], "");pass(&mut g);assert_eq!(g.stack.len(),1);pass(&mut g);
    assert_eq!(g.battlefield.iter().filter(|o|g.obj(**o).token()).count(),1);
}
#[test]
fn graha_impossible_payment_leaves_allowance_and_spell_x_is_captured_before_departure(){
    let mut g=setup("G'raha Tia, Scion Reborn");g.players[0].pool.colorless=3;g.lose_life(0,19).unwrap();
    cast(&mut g,3,vec![], "GGG");pass(&mut g);assert!(g.decision.is_none());
    cite!(["119.4"]; assert_eq!(g.battlefield.iter().filter(|o|g.obj(**o).token()).count(),0));
    pass(&mut g);g.gain_life(0,10).unwrap();g.players[0].pool.colorless=4;
    let card=g.put(0,5,Zone::Hand);g.act(0,paid("CCCC",Move::WithX(4,Box::new(Move::Cast{card,targets:vec![]})))).unwrap();
    let spell=g.stack[0];g.move_object(spell,Zone::Graveyard);
    let source=g.battlefield[0];g.move_object(source,Zone::Hand);pass(&mut g);
    cite!(["603.2h"]; assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::Pay{life:4,..},..})),"{:?}",g.decision));
    g.act(0,Move::Choose(Answer::Pay { plan: vec![], with: None, tapped: vec![] })).unwrap();let hero=*g.battlefield.iter().find(|o|g.obj(**o).token()).unwrap();
    cite!(["608.2h"]; assert_eq!(g.obj(hero).counters.get("+1/+1"),Some(&4)));
}
#[test]
fn tataru_illegal_entry_target_stops_the_controllers_draw_too(){
    let mut g=setup("Tataru Taru");let source=g.battlefield[0];g.move_object(source,Zone::Hand);
    let card=g.players[0].hand[0];g.move_object(card,Zone::Battlefield);g.act(0,Move::Pass).unwrap();
    g.act(0,Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();let before=g.players[0].hand.len();
    g.lose_life(1,100).unwrap();
    for _ in 0..8{if g.stack.is_empty(){break;}g.act(g.priority,Move::Pass).unwrap();}
    assert!(g.stack.is_empty());cite!(["608.2b"]; assert_eq!(g.players[0].hand.len(),before));
}
