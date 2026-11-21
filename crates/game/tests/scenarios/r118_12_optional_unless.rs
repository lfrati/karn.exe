//! Unless payment precedes the optional action it gates (118.12a; Rhystic Study ruling).
use crate::support::{cite, mana_pool};
use game::{CardDef,Game,Move,Zone};
use game::state::{Answer,Choice,Decision};
fn setup(mana:bool)->Game {
    let defs=vec![
        CardDef::new("Rhystic Study","{2}{U}","Enchantment","Whenever an opponent casts a spell, you may draw a card unless that player pays {1}.",None).unwrap(),
        CardDef::new("Spell","{0}","Instant","You gain 1 life.",None).unwrap(),
        CardDef::new("Forest","","Basic Land — Forest","",None).unwrap(),
    ];
    for def in &defs {game::play::check(def).unwrap();}
    let mut g=Game::new(defs,3,20);g.put(0,0,Zone::Battlefield);g.put(0,2,Zone::Library);
    g.act(0,Move::Pass).unwrap();let spell=g.put(1,1,Zone::Hand);
    g.act(1,Move::Cast{card:spell,targets:vec![]}).unwrap();
    if mana {g.put(1,2,Zone::Battlefield);}
    for _ in 0..3 {g.act(g.priority,Move::Pass).unwrap();}
    g
}
#[test]
fn payer_chooses_before_controller_may_draw_and_payment_stops_draw() {
    let mut g=setup(true);
    cite!(["118.12a"]; assert!(matches!(g.decision,Some(Decision::Choose{player:1,choice:Choice::Pay{..}}))));
    cite!(["118.12a"]; refused("that decision is another player's"): g.act(0,Move::Choose(Answer::No)));
    cite!(["118.12a"]; refused("that decision is another player's"): g.act(2,Move::Choose(Answer::Yes)));
    let step = game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source: g.battlefield[1], ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None };
    g.act(1,Move::Choose(Answer::Pay { plan: vec![step], with: Some(mana_pool("G")), tapped: vec![] })).unwrap();
    assert!(g.decision.is_none());cite!(["118.12a"]; assert!(g.players[0].hand.is_empty()));
    assert_eq!(g.stack.len(),1,"Study resolves before the spell");
}
#[test]
fn declined_payment_then_optional_draw_can_each_be_declined_or_accepted() {
    for draw in [false,true] {
        let mut g=setup(true);g.act(1,Move::Choose(Answer::No)).unwrap();
        cite!(["118.12a"]; assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::May}))));
        crate::support::refused(g.act(1,Move::Choose(Answer::Yes)), "that decision is another player's");
        let mut copy=g.clone();
        for game in [&mut g,&mut copy] {
            game.act(0,Move::Choose(if draw{Answer::Yes}else{Answer::No})).unwrap();
            cite!(["118.12a"]; assert_eq!(game.players[0].hand.len(),usize::from(draw)));assert_eq!(game.stack.len(),1);
        }
    }
}
#[test]
fn declined_unaffordable_payment_still_allows_the_optional_draw() {
    let mut g=setup(false);
    let before = format!("{g:?}");
    cite!(["118.3"]; refused("that mana is not in your mana pool"): g.act(1,Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool("G")), tapped: vec![] })));
    assert_eq!(format!("{g:?}"), before);
    g.act(1,Move::Choose(Answer::No)).unwrap();
    cite!(["118.3","118.12a"]; assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::May}))));
    g.act(0,Move::Choose(Answer::No)).unwrap();assert!(g.players[0].hand.is_empty());
}
#[test]
fn a_multiple_player_unless_payment_is_refused_instead_of_using_only_the_first_payer() {
    let def=CardDef::new("Refused","{1}","Sorcery","You may draw a card unless each player pays {1}.",None).unwrap();
    assert_eq!(game::play::check(&def).expect_err("multiple payers are unsupported").0,"unless: several players need per-player payments");
}

#[test]
fn singular_they_payment_retains_the_bound_target_controllers_cardinality() {
    let def=CardDef::new("Hope-Ender Coatl","{2}{U}","Creature — Eldrazi Snake",
        "Devoid (This card has no color.)\nFlash\nWhen you cast this spell, counter target spell an opponent controls unless they pay {1}.\nFlying",Some((2,1))).unwrap();
    game::play::check(&def).unwrap();
}
