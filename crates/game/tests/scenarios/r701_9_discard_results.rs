//! Actual completed discards feed identified result quantities (701.9,608.2c,614.6).
use crate::support::cite;
use game::{CardDef,Game,Move,Zone,Event};
use game::state::{Answer,Choice,Decision};
const SYPHON:&str="Each other player discards a card. You draw a card for each card discarded this way.";
fn game(n:usize)->Game {
    let defs=vec![
        CardDef::new("Syphon Mind","{3}{B}","Sorcery",SYPHON,None).unwrap(),
        CardDef::new("Bear","{G}","Creature — Bear","",Some((2,2))).unwrap(),
        CardDef::new("Replaced","{G}","Creature — Bear","If a spell or ability an opponent controls causes you to discard this card, put it onto the battlefield instead of putting it into your graveyard.",Some((2,2))).unwrap(),
        CardDef::new("Competing","{G}","Creature — Bear","If a spell or ability an opponent controls causes you to discard this card, put it onto the battlefield instead of putting it into your graveyard.\nMadness {G}",Some((2,2))).unwrap(),
        CardDef::new("Entry restriction","{1}","Artifact","Creature cards in hands can't enter the battlefield.",None).unwrap(),
    ];
    for def in &defs {game::play::check(def).unwrap_or_else(|e|panic!("{}: {}",def.name(),e.0));}
    Game::new(defs,n,20)
}
fn cast(g:&mut Game) {
    for _ in 0..4 {g.put(0,1,Zone::Library);}
    let card=g.put(0,0,Zone::Hand);g.players[0].pool.colorless=3;g.players[0].pool.colors[2]=1;
    g.act(0,crate::support::paid("CCCB", Move::Cast{card,targets:vec![]})).unwrap();for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}
}
#[test]
fn full_syphon_collects_private_choices_before_any_discard_and_counts_nonempty_hands() {
    let mut g=game(4);let a=g.put(1,1,Zone::Hand);g.put(1,1,Zone::Hand);let b=g.put(2,1,Zone::Hand);g.put(2,1,Zone::Hand);
    cast(&mut g);cite!(["701.9b"]; assert!(matches!(g.decision,Some(Decision::Choose{player:1,choice:Choice::Objects{..}}))));
    g.act(1,Move::Choose(Answer::Objects(vec![a]))).unwrap();
    cite!(["101.4"]; assert!(g.players[1].hand.contains(&a)));assert!(g.players[1].graveyard.is_empty());
    cite!(["101.4"]; assert!(matches!(g.decision,Some(Decision::Choose{player:2,choice:Choice::Objects{..}}))));
    let mut copy=g.clone();for game in [&mut g,&mut copy] {
        game.retire_events(game.log.end());game.act(2,Move::Choose(Answer::Objects(vec![b]))).unwrap();
        cite!(["608.2c"]; assert_eq!(game.players[0].hand.len(),2));assert_eq!(game.players[0].library.len(),2);
        assert_eq!(game.log.iter().filter(|e|matches!(e,Event::Discarded{..})).count(),2);
    }
}
#[test]
fn replaced_destinations_count_as_discards_but_prevented_entry_does_not() {
    for prohibit in [false,true] {
        let mut g=game(2);let card=g.put(1,2,Zone::Hand);if prohibit {g.put(0,4,Zone::Battlefield);}
        cast(&mut g);
        cite!(["608.2c"]; case: prohibit; assert_eq!(g.players[0].hand.len(),usize::from(!prohibit)));
        cite!(["614.6"]; case: prohibit; assert_eq!(g.players[1].hand.contains(&card),prohibit));
        assert_eq!(g.players[1].graveyard.len(),0);
        assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Discarded{..})).count(),usize::from(!prohibit));
    }
}
#[test]
fn replacement_question_clones_finish_the_discard_once_before_counting() {
    let mut g=game(2);let card=g.put(1,3,Zone::Hand);cast(&mut g);
    let Some(Decision::Choose{player:1,choice:Choice::Replacement{among,..}})=&g.decision else {panic!("{:?}",g.decision)};
    let battlefield=among.iter().find(|e|e.ability().unwrap().index==0).unwrap().clone();
    let mut copy=g.clone();for game in [&mut g,&mut copy] {
        game.act(1,Move::Choose(Answer::Replacement(battlefield.clone()))).unwrap();
        cite!(["608.2c"]; assert_eq!(game.players[0].hand.len(),1));assert!(game.players[1].hand.is_empty());
        assert_eq!(game.log.iter().filter(|e|matches!(e,Event::Discarded{..})).count(),1);
        cite!(["614.1a"]; assert!(game.battlefield.iter().any(|o|game.obj(*o).card==Some(3))));
        assert!(!game.players[1].hand.contains(&card));
    }
}
#[test]
fn empty_hands_and_ambiguous_or_missing_producers_have_no_invented_result() {
    let mut g=game(3);cast(&mut g);cite!(["608.2c"]; assert!(g.players[0].hand.is_empty()));assert!(g.decision.is_none());
    for text in ["You draw a card for each card discarded this way.","Each opponent discards a card. Each opponent discards a card. You draw a card for each card discarded this way."] {
        let def=CardDef::new("Refused","{0}","Sorcery",text,None).unwrap();
        crate::support::refused(game::play::check(&def).map_err(|e|e.0),"result quantity: absent or ambiguous action-result producer");
    }
}
