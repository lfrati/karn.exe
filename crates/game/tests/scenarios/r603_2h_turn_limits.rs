//! Trigger caps and action caps consume different moments (603.2h).
use crate::support::cite;
use game::{CardDef, Game, Move, Zone};
use game::state::{Answer, Choice, Decision};

fn setup(action: bool) -> Game {
    let text = if action {
        "Whenever you cast a spell, you may pay {0}. If you do, you gain 1 life. Do this only once each turn."
    } else {
        "Whenever you cast a spell, you gain 1 life. This ability triggers only once each turn."
    };
    let defs = vec![CardDef::new("Limited", "{0}", "Enchantment", text, None).unwrap(),
        CardDef::new("Spell", "{0}", "Instant", "You gain 1 life.", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Steal", "{0}", "Instant", "You gain control of target permanent.", None).unwrap()];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {e:?}", d.name())); }
    let mut g = Game::new(defs, 3, 20);g.put(0, 0, Zone::Battlefield);
    for p in 0..3 { for _ in 0..10 {g.put(p, 2, Zone::Library);} }
    g
}
fn cast(g: &mut Game) {
    while g.priority != 0 {g.act(g.priority, Move::Pass).unwrap();}
    let card=g.put(0,1,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![]}).unwrap();
}
fn pass(g: &mut Game) {for _ in 0..3 {g.act(g.priority,Move::Pass).unwrap();}}
fn pay(g: &mut Game, yes: bool) {
    assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Pay{..}})));
    g.act(0,Move::Choose(if yes {Answer::Pay { plan: vec![], with: None, tapped: vec![] }} else {Answer::No})).unwrap();
}
#[test]
fn trigger_cap_is_consumed_before_resolution_and_resets_on_the_next_players_turn() {
    let mut g=setup(false);cast(&mut g);cast(&mut g);
    // Uncited (card text): "This ability triggers only once each turn." (603.2h covers only "Do this only once each turn").
    assert_eq!(g.stack.len(),3,"only one trigger above two spells");
    // Uncited (card text): "This ability triggers only once each turn."
    pass(&mut g);assert_eq!(g.players[0].life,21);
    pass(&mut g);pass(&mut g);
    for _ in 0..80 {
        if g.active==1 {break;}
        let p=g.priority;
        if matches!(g.decision,Some(Decision::Attackers{..})) {g.act(p,Move::Attack(vec![])).unwrap();}
        else {g.act(p,Move::Pass).unwrap();}
    }
    // Uncited (card text): "only once each turn" is counted per turn, so the next turn's cast triggers again.
    assert_eq!(g.active,1);cast(&mut g);assert_eq!(g.stack.len(),2);
}
#[test]
fn action_cap_allows_pending_instances_but_only_one_successful_action() {
    let mut g=setup(true);cast(&mut g);cast(&mut g);cite!(["603.2h"]; assert_eq!(g.stack.len(),4));
    pass(&mut g);let mut clone=g.clone();
    for game in [&mut g,&mut clone] {
        pay(game,true);assert_eq!(game.players[0].life,21);
        pass(game);assert_eq!(game.players[0].life,22,"original spell resolves");
        // Uncited (card text): "Do this only once each turn" limits the action itself; 603.2h decides only whether the ability triggers, not an instance already pending.
        pass(game);assert!(game.decision.is_none(),"earlier pending trigger cannot ask again");
        // Uncited (card text): "Do this only once each turn."
        assert_eq!(game.players[0].life,22);
        cast(game);cite!(["603.2h"]; assert_eq!(game.stack.len(),2,"used source no longer triggers"));
    }
}
#[test]
fn declining_leaves_pending_and_future_instances_eligible() {
    let mut g=setup(true);cast(&mut g);cast(&mut g);pass(&mut g);pay(&mut g,false);
    pass(&mut g);pass(&mut g);pay(&mut g,true);cite!(["603.2h"]; assert_eq!(g.players[0].life,22));
}
#[test]
fn new_incarnation_has_its_own_action_allowance() {
    let mut g=setup(true);let source=g.battlefield[0];cast(&mut g);pass(&mut g);pay(&mut g,true);
    g.move_object(source,Zone::Hand);let card=g.players[0].hand[0];g.move_object(card,Zone::Battlefield);
    while !g.stack.is_empty(){pass(&mut g);}
    // the limit is of the ability it is attached to (Riveteers Ascendancy ruling): a new object has its own
    cast(&mut g);pass(&mut g);pay(&mut g,true);cite!(["603.2h","400.7"]; assert_eq!(g.players[0].life,23));
}

#[test]
fn controller_change_preserves_trigger_cap_but_has_a_separate_action_actor(){
    for action in [false,true]{
        let mut g=setup(action);let source=g.battlefield[0];cast(&mut g);pass(&mut g);
        if action{pay(&mut g,true);}
        pass(&mut g);while g.priority!=1{g.act(g.priority,Move::Pass).unwrap();}
        let steal=g.put(1,3,Zone::Hand);g.act(1,Move::Cast{card:steal,targets:vec![vec![game::Target::Object(source)]]}).unwrap();pass(&mut g);
        assert_eq!(g.controller(source),1);while g.priority!=1{g.act(g.priority,Move::Pass).unwrap();}
        let card=g.put(1,1,Zone::Hand);g.act(1,Move::Cast{card,targets:vec![]}).unwrap();
        if action{cite!(["603.2h"]; assert_eq!(g.stack.len(),2));}
        // Uncited (card text): "This ability triggers only once each turn" counts the ability's triggers that turn, whoever controls it.
        else{assert_eq!(g.stack.len(),1);}
        if action{pass(&mut g);cite!(["603.2h", "603.3a"]; assert!(matches!(g.decision,Some(Decision::Choose{player:1,choice:Choice::Pay{..}}))));}
    }
}

#[test]
fn an_unrelated_unless_payer_is_not_the_limited_action_producer(){
    let def=CardDef::new("Refused","{0}","Enchantment",
        "Whenever an opponent casts a spell, you may draw a card unless that player pays {1}. Do this only once each turn.",None).unwrap();
    // Uncited (admission): an unrelated unless-payer cannot consume the controller action cap.
    crate::support::refused(game::play::check(&def).map_err(|e|e.0), "action limit: requires one identified controller payment or cast");
}
