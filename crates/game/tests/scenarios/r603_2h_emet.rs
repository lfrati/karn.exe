//! Full Emet uses grouped life loss, ordinary targeted graveyard casting and an exile rider.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Zone, Target};
use game::state::{Answer, Choice, Decision};
fn setup()->Game{
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let defs=vec![CardDef::from_face(faces.iter().find(|f|f.face_name=="Emet-Selch of the Third Seat").unwrap()).unwrap(),
        CardDef::new("Spell","{3}","Sorcery","You gain 1 life.",None).unwrap(),
        CardDef::new("Loss","{0}","Instant","Each opponent loses 1 life.",None).unwrap(),
        CardDef::new("Sequential","{0}","Instant","Target opponent loses 1 life. Target opponent loses 1 life.",None).unwrap(),
        CardDef::new("Forest","","Basic Land — Forest","",None).unwrap()];
    for d in &defs{game::play::check(d).unwrap_or_else(|e|panic!("{}: {e:?}",d.name()));}
    let mut g=Game::new(defs,3,20);g.put(0,0,Zone::Battlefield);g.put(0,1,Zone::Graveyard);
    for p in 0..3{for _ in 0..10{g.put(p,4,Zone::Library);}}
    g
}
fn pass(g:&mut Game){for _ in 0..3{g.act(g.priority,Move::Pass).unwrap();}}
fn loss(g:&mut Game){
    while g.priority!=0{g.act(g.priority,Move::Pass).unwrap();}
    let card=g.put(0,2,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![]}).unwrap();pass(g);
}
fn target(g:&mut Game){
    assert!(matches!(g.decision,Some(Decision::Targets{player:0,..})),"{:?}",g.decision);
    let card=g.players[0].graveyard.iter().find(|o|g.obj(**o).card==Some(1)).copied().unwrap();
    g.act(0,Move::ChooseTargets(vec![vec![Target::Object(card)]])).unwrap();
}
fn offered(g:&Game)->game::ObjId{
    let Some(Decision::Choose{player:0,choice:Choice::Cast{cards}})=&g.decision else{panic!("{:?}",g.decision)};
    assert_eq!(cards.len(),1);cards[0]
}
#[test]
fn simultaneous_opponent_life_loss_is_one_targeted_offer_decline_does_not_consume(){
    let mut g=setup();loss(&mut g);target(&mut g);cite!(["603.2c"]; assert_eq!(g.stack.len(),1));
    pass(&mut g);offered(&g);g.act(0,Move::Choose(Answer::No)).unwrap();
    loss(&mut g);cite!(["603.2h"]; assert!(matches!(g.decision,Some(Decision::Targets{player:0,..})),"{:?}",g.decision));target(&mut g);pass(&mut g);offered(&g);
}
#[test]
fn successful_graveyard_cast_gets_discount_and_exile_rider_and_consumes_action(){
    let mut g=setup();loss(&mut g);target(&mut g);pass(&mut g);let card=offered(&g);
    cite!(["601.2f"]; refused("that mana is not in your mana pool (118.3a)"): g.act(0,paid("C",Move::Cast{card,targets:vec![]})));
    assert_eq!(offered(&g),card);g.players[0].pool.colorless=1;
    let mut clone=g.clone();for game in [&mut g,&mut clone]{
        game.act(0,paid("C",Move::Cast{card,targets:vec![]})).unwrap();cite!(["601.2f"]; assert_eq!(game.players[0].pool.total(),0));
        assert_eq!(game.stack.len(),1);pass(game);assert_eq!(game.players[0].life,21);
        assert!(game.exile.iter().any(|o|game.obj(*o).card==Some(1)));
        game.put(0,1,Zone::Graveyard);loss(game);cite!(["603.2h"]; assert!(game.decision.is_none()));assert!(game.stack.is_empty());
    }
}
#[test]
fn separate_life_loss_actions_queue_two_instances_but_only_one_can_cast(){
    let mut g=setup();let card=g.put(0,3,Zone::Hand);
    g.act(0,Move::Cast{card,targets:vec![vec![Target::Player(1)],vec![Target::Player(2)]]}).unwrap();pass(&mut g);
    let Some(Decision::Order{player:0,abilities})=g.decision.clone() else {panic!("expected ordering for the two life-loss subjects: {:?}",g.decision)};
    cite!(["603.3b"]; assert_eq!(abilities.len(),2));
    // Both target the same graveyard card; choose their order explicitly before targets.
    g.act(0,Move::Order(vec![0,1])).unwrap();
    target(&mut g);target(&mut g);cite!(["603.2c"]; assert_eq!(g.stack.len(),2));pass(&mut g);
    let card=offered(&g);g.players[0].pool.colorless=1;g.act(0,paid("C",Move::Cast{card,targets:vec![]})).unwrap();
    pass(&mut g);g.put(0,1,Zone::Graveyard);pass(&mut g);cite!(["603.2h"]; assert!(g.decision.is_none()));assert!(g.stack.is_empty());
}
#[test]
fn departed_source_still_offers_cast_but_its_static_discount_is_gone(){
    let mut g=setup();loss(&mut g);target(&mut g);let source=g.battlefield[0];g.move_object(source,Zone::Hand);
    pass(&mut g);let card=offered(&g);g.players[0].pool.colorless=1;
    // The available C would pay the discounted cost, but cannot pay the full3 after Emet leaves.
    let before=format!("{g:?}");
    cite!(["611.3a", "601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0,paid("C",Move::Cast{card,targets:vec![]})));
    cite!(["733.1"]; assert_eq!(format!("{g:?}"),before));g.players[0].pool.colorless=3;
    cite!(["611.3a"]; ok: g.act(0,paid("CCC",Move::Cast{card,targets:vec![]})));pass(&mut g);assert!(g.exile.iter().any(|o|g.obj(*o).card==Some(1)));
}
#[test]
fn illegal_target_does_not_use_action_and_countering_cast_still_applies_exile_rider(){
    let mut g=setup();loss(&mut g);target(&mut g);let card=g.players[0].graveyard.iter().find(|o|g.obj(**o).card==Some(1)).copied().unwrap();
    g.move_object(card,Zone::Hand);pass(&mut g);cite!(["608.2b"]; assert!(g.decision.is_none()));
    let hand=g.players[0].hand.iter().find(|o|g.obj(**o).card==Some(1)).copied().unwrap();g.move_object(hand,Zone::Graveyard);
    loss(&mut g);target(&mut g);pass(&mut g);let card=offered(&g);g.players[0].pool.colorless=1;
    g.retire_events(g.log.end());g.act(0,paid("C",Move::Cast{card,targets:vec![]})).unwrap();let spell=g.stack[0];g.move_object(spell,Zone::Graveyard);
    assert!(g.exile.iter().any(|o|g.obj(*o).card==Some(1)));g.put(0,1,Zone::Graveyard);loss(&mut g);cite!(["603.2h"]; assert!(g.decision.is_none()));
}
