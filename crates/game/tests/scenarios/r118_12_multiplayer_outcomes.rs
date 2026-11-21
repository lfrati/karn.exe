//! Per-player payment decisions share one simultaneous object action (101.4,118.12,608.2e).
use crate::support::cite;
use game::{CardDef,Game,Move,Zone,Event};
use game::state::{Answer,Choice,Decision};
const FANDANIEL:&str="Whenever you cast an instant or sorcery spell, surveil 1.\nAt the beginning of your end step, each opponent may sacrifice a nontoken creature of their choice. Each opponent who doesn't loses 2 life for each instant and sorcery card in your graveyard.";
fn game(n:usize)->Game {
    let defs=vec![
        CardDef::new("Fandaniel, Telophoroi Ascian","{4}{B}","Legendary Creature — Elder Wizard",FANDANIEL,Some((4,5))).unwrap(),
        CardDef::new("Bear","{G}","Creature — Bear","",Some((2,2))).unwrap(),
        CardDef::new("Spell","{0}","Instant","You gain 1 life.",None).unwrap(),
        CardDef::new("Replaced creature","{G}","Creature — Bear","If this creature would die, put it on the bottom of its owner's library instead.",Some((2,2))).unwrap(),
    ];
    for d in &defs {game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}
    Game::new(defs,n,20)
}
fn end(g:&mut Game) {
    while g.part!=ir::Part::End {
        let mv=if matches!(g.decision,Some(Decision::Attackers{..})) {Move::Attack(vec![])} else {Move::Pass};
        g.act(g.priority,mv).unwrap();
    }
    for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}
}
fn may(g:&Game,p:usize) {assert!(matches!(g.decision,Some(Decision::Choose{player,choice:Choice::May}) if player==p),"{:?}",g.decision);}
#[test]
fn mixed_opponents_choose_before_sacrifice_and_each_failure_loses_life() {
    let mut g=game(4);g.put(0,0,Zone::Battlefield);g.put(0,2,Zone::Graveyard);g.put(0,2,Zone::Graveyard);
    let a=g.put(1,1,Zone::Battlefield);g.put(1,1,Zone::Battlefield);let b=g.put(2,1,Zone::Battlefield);end(&mut g);
    cite!(["101.4","608.2e"]; assert!(matches!(g.decision,Some(Decision::Choose{player:1,choice:Choice::May})),"{:?}",g.decision));crate::support::refused(g.act(2,Move::Choose(Answer::Yes)), "that decision is another player's");
    g.act(1,Move::Choose(Answer::Yes)).unwrap();
    assert!(matches!(g.decision,Some(Decision::Choose{player:1,choice:Choice::Objects{..}})));
    let before=format!("{g:?}");
    cite!(["608.2d"]; refused("choose exactly 1 of the offered objects to sacrifice (608.2d)"): g.act(1,Move::Choose(Answer::Yes)));
    crate::support::refused(g.act(2,Move::Choose(Answer::Objects(vec![a]))), "that decision is another player's");
    assert_eq!(format!("{g:?}"),before,"refused answers preserve both consent and selection progress");
    g.act(1,Move::Choose(Answer::Objects(vec![a]))).unwrap();cite!(["101.4"]; assert!(matches!(g.decision,Some(Decision::Choose{player:2,choice:Choice::May})),"{:?}",g.decision));
    cite!(["101.4","608.2e"]; assert!(g.battlefield.contains(&a),"all selections precede mutation"));
    let mut copy=g.clone();
    for game in [&mut g,&mut copy] {
        game.retire_events(game.log.end());game.act(2,Move::Choose(Answer::No)).unwrap();
        assert!(!game.battlefield.contains(&a));assert!(game.battlefield.contains(&b));
        cite!(["118.12"]; assert_eq!(game.players.iter().map(|p|p.life).collect::<Vec<_>>(),vec![20,20,16,16]));
        assert_eq!(game.log.iter().filter(|e|matches!(e,Event::Sacrificed{..})).count(),1);
    }
}
#[test]
fn impossible_first_opponent_does_not_suppress_later_choice_and_tokens_are_excluded() {
    let mut g=game(3);g.put(0,0,Zone::Battlefield);g.put(0,2,Zone::Graveyard);
    let original=g.put(1,1,Zone::Hand);let chars=g.obj(original).chars.clone();
    let token=g.create_token(1,chars,false);let b=g.put(2,1,Zone::Battlefield);end(&mut g);
    cite!(["608.2d"]; assert!(matches!(g.decision,Some(Decision::Choose{player:2,choice:Choice::May})),"{:?}",g.decision));g.act(2,Move::Choose(Answer::Yes)).unwrap();
    assert!(g.battlefield.contains(&token));assert!(!g.battlefield.contains(&b));
    cite!(["118.12"]; assert_eq!(g.players[1].life,18));assert_eq!(g.players[2].life,20);
}
#[test]
fn a_replaced_sacrifice_still_pays_and_choices_remain_simultaneous() {
    let mut g=game(3);g.put(0,0,Zone::Battlefield);g.put(0,2,Zone::Graveyard);
    let a=g.put(1,3,Zone::Battlefield);let b=g.put(2,3,Zone::Battlefield);end(&mut g);
    may(&g,1);g.act(1,Move::Choose(Answer::Yes)).unwrap();may(&g,2);
    cite!(["608.2e"]; assert!(g.battlefield.contains(&a)));g.act(2,Move::Choose(Answer::Yes)).unwrap();
    cite!(["614.1a"]; assert_eq!((g.players[1].library.len(),g.players[2].library.len()),(1,1)));
    assert!(g.players[1].graveyard.is_empty()&&g.players[2].graveyard.is_empty());
    assert!(!g.battlefield.contains(&b));cite!(["118.12"]; assert_eq!((g.players[1].life,g.players[2].life),(20,20)));
}
#[test]
fn surveil_occurs_before_its_spell_and_the_private_choice_survives_cloning() {
    let mut g=game(3);g.put(0,0,Zone::Battlefield);let top=g.put(0,1,Zone::Library);let spell=g.put(0,2,Zone::Hand);
    g.act(0,Move::Cast{card:spell,targets:vec![]}).unwrap();
    for _ in 0..3 {g.act(g.priority,Move::Pass).unwrap();}
    assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Arrange{verb:ir::Verb::Surveil,..}})));
    let mut copy=g.clone();for game in [&mut g,&mut copy] {
        game.act(0,Move::Choose(Answer::Arrange{top:vec![],away:vec![top]})).unwrap();
        assert_eq!(game.stack.len(),1);cite!(["701.25a"]; assert_eq!(game.players[0].graveyard.len(),1));
        cite!(["603.3"]; assert_eq!(game.players[0].life,20,"spell has not resolved"));
    }
}
#[test]
fn absent_or_ambiguous_per_player_payment_producers_are_refused() {
    for text in ["Each opponent who doesn't loses 2 life.","Each opponent may sacrifice a creature. Each opponent may discard a card. Each opponent who doesn't loses 2 life."] {
        let def=CardDef::new("Refused","{0}","Sorcery",text,None).unwrap();
        assert_eq!(game::play::check(&def).expect_err("payment producer must be unique").0,"player outcome: absent or ambiguous object-payment producer", "{text}");
    }
}

#[test]
fn per_player_impossibility_is_distinct_from_declining_a_possible_payment() {
    let spell=CardDef::new("Possible","{0}","Sorcery","Each opponent may sacrifice a creature. Each opponent who can't loses 3 life.",None).unwrap();
    game::play::check(&spell).unwrap();
    let bear=CardDef::new("Bear","{G}","Creature — Bear","",Some((2,2))).unwrap();
    let mut g=Game::new(vec![spell,bear],3,20);g.put(1,1,Zone::Battlefield);let card=g.put(0,0,Zone::Hand);
    g.act(0,Move::Cast{card,targets:vec![]}).unwrap();for _ in 0..3 {g.act(g.priority,Move::Pass).unwrap();}
    may(&g,1);g.act(1,Move::Choose(Answer::No)).unwrap();
    cite!(["118.12"]; assert_eq!((g.players[1].life,g.players[2].life),(20,17)));
}
