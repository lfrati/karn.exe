//! CR 101.2, 118.12, 701.21: prohibitions follow the causing effect's controller.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};

fn card(name: &str, types: &str, text: &str, pt: Option<(i32,i32)>) -> CardDef {
    let d=CardDef::new(name,"{0}",types,text,pt).unwrap();
    game::play::check(&d).unwrap_or_else(|e|panic!("{name}: {}",e.0));d
}
fn preserver() -> CardDef {
    static CARD: std::sync::OnceLock<CardDef> = std::sync::OnceLock::new();
    CARD.get_or_init(|| {
        let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        let d=CardDef::from_face(faces.iter().find(|f|f.name=="Tajuru Preserver").unwrap()).unwrap();
        game::play::check(&d).unwrap();d
    }).clone()
}
fn game(effect: CardDef, seats: usize) -> (Game, ObjId, ObjId) {
    let mut g=Game::new(vec![preserver(),card("Bear","Creature — Bear","",Some((2,2))),effect],seats,20);
    let preserver=g.put(0,0,Zone::Battlefield);let bear=g.put(0,1,Zone::Battlefield);(g,preserver,bear)
}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
fn priority(g:&mut Game, player:usize){while g.priority!=player{g.act(g.priority,Move::Pass).unwrap();}}
fn cast(g:&mut Game,player:usize,targets:Vec<Vec<Target>>){
    priority(g,player);let spell=g.put(player,2,Zone::Hand);g.act(player,Move::Cast{card:spell,targets}).unwrap();
}
fn sacrificed(g:&Game,player:usize)->Vec<ObjId>{
    g.log.iter().filter_map(|e|match e{Event::Sacrificed{player:p,object}if *p==player=>Some(*object),_=>None}).collect()
}

#[test]
fn r101_2_full_tajuru_preserver_stops_opposing_spells_but_not_own_effects(){
    for controller in [0,1] {
        let (mut g,preserver,bear)=game(card("Edict","Instant","Target player sacrifices a creature.",None),2);
        cast(&mut g,controller,vec![vec![Target::Player(0)]]);pass(&mut g);
        if controller==0 {
            assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Objects{..}})));
            g.act(0,Move::Choose(Answer::Objects(vec![bear]))).unwrap();
            cite!(["701.21a"]; assert_eq!(sacrificed(&g,0),vec![bear]));
        }else{assert!(g.decision.is_none());cite!(["101.2"]; assert!(sacrificed(&g,0).is_empty()));assert!(g.battlefield.contains(&bear));}
        assert!(g.battlefield.contains(&preserver));assert!(g.stack.is_empty());
    }
}

#[test]
fn r609_3_each_player_effect_skips_only_the_protected_player_and_continues(){
    let (mut g,preserver,bear)=game(card("Demand","Instant","Each player sacrifices a creature. You gain 2 life.",None),3);
    let one=g.put(1,1,Zone::Battlefield);let two=g.put(2,1,Zone::Battlefield);
    cast(&mut g,1,vec![]);pass(&mut g);
    assert_eq!(g.battlefield,vec![preserver,bear]);assert_eq!(sacrificed(&g,1),vec![one]);assert_eq!(sacrificed(&g,2),vec![two]);
    cite!(["101.2"]; assert!(sacrificed(&g,0).is_empty()));cite!(["609.3"]; assert_eq!(g.players[1].life,22));
}

#[test]
fn r113_7a_opposing_activated_ability_retains_its_cause_after_source_departure_and_clone(){
    let (mut g,preserver,bear)=game(card("Altar","Artifact","Sacrifice this artifact: Target player sacrifices a creature.",None),2);
    let source=g.put(1,2,Zone::Battlefield);priority(&mut g,1);
    g.act(1,Move::Activate{source,ability:0,targets:vec![vec![Target::Player(0)]],paying:vec![]}).unwrap();
    assert!(!g.battlefield.contains(&source));assert_eq!(sacrificed(&g,1),vec![source]);
    let clone=g.clone();for (case,mut branch) in [("original",g),("clone",clone)]{pass(&mut branch);assert_eq!(branch.battlefield,vec![preserver,bear]);cite!(["113.7a","101.2"]; case: case; assert!(sacrificed(&branch,0).is_empty()));}
}

#[test]
fn r603_3_opposing_trigger_retains_its_cause_after_source_departure(){
    let (mut g,preserver,bear)=game(card("Witness","Artifact","Flash\nWhen this artifact enters, each opponent sacrifices a creature.",None),2);
    cast(&mut g,1,vec![]);pass(&mut g);assert_eq!(g.stack.len(),1);
    let source=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(2)).unwrap();g.destroy(source);
    pass(&mut g);assert_eq!(g.battlefield,vec![preserver,bear]);cite!(["113.7a","101.2"]; assert!(sacrificed(&g,0).is_empty()));
}

#[test]
fn r604_2_prohibition_ends_when_preserver_leaves_before_resolution(){
    let (mut g,preserver,bear)=game(card("Edict","Instant","Target player sacrifices a creature.",None),2);
    cast(&mut g,1,vec![vec![Target::Player(0)]]);let clone=g.clone();g.destroy(preserver);pass(&mut g);
    assert!(g.battlefield.is_empty());cite!(["604.2"]; assert_eq!(sacrificed(&g,0),vec![bear]));
    let mut clone=clone;pass(&mut clone);cite!(["101.2"]; assert_eq!(clone.battlefield,vec![preserver,bear]));
}

#[test]
fn r118_12_prohibited_optional_sacrifice_is_not_offered_and_takes_the_cannot_pay_branch(){
    for noun in ["a creature","all creatures"] {
        let text=format!("Target player may sacrifice {noun}. If they don't, they lose 3 life.");
        let (mut g,preserver,bear)=game(card("Offer","Instant",&text,None),2);
        cast(&mut g,1,vec![vec![Target::Player(0)]]);pass(&mut g);
        assert!(g.decision.is_none(),"{noun}: {:?}",g.decision);assert!(g.stack.is_empty());
        cite!(["118.12"]; case: noun; assert_eq!(g.players[0].life,17));assert_eq!(g.battlefield,vec![preserver,bear]);cite!(["101.2"]; case: noun; assert!(sacrificed(&g,0).is_empty()));
    }
}

#[test]
fn r118_12_impossible_mandatory_payment_takes_the_cannot_branch(){
    let (mut g,preserver,bear)=game(card("Demand","Instant","Target player sacrifices a creature. If they can't, they lose 3 life.",None),2);
    cast(&mut g,1,vec![vec![Target::Player(0)]]);pass(&mut g);
    assert!(g.decision.is_none()&&g.stack.is_empty());cite!(["118.12"]; assert_eq!(g.players[0].life,17));
    assert_eq!(g.battlefield,vec![preserver,bear]);assert!(sacrificed(&g,0).is_empty());
}

#[test]
fn r118_12_own_optional_sacrifice_still_offers_payment_and_branches_on_actual_payment(){
    let (mut g,_,bear)=game(card("Offer","Instant","You may sacrifice a creature. If you do, you gain 3 life.",None),2);
    cast(&mut g,0,vec![]);pass(&mut g);assert_eq!(g.decision,Some(Decision::Choose{player:0,choice:Choice::May}));
    let mut declined=g.clone();declined.act(0,Move::Choose(Answer::No)).unwrap();cite!(["118.12"]; assert_eq!(declined.players[0].life,20));
    g.act(0,Move::Choose(Answer::Yes)).unwrap();g.act(0,Move::Choose(Answer::Objects(vec![bear]))).unwrap();
    cite!(["118.12"]; assert_eq!(g.players[0].life,23));assert_eq!(sacrificed(&g,0),vec![bear]);
}

#[test]
fn r118_10_announced_activation_and_casting_costs_are_not_opposing_effects(){
    let (mut g,preserver,bear)=game(card("Altar","Artifact","Sacrifice a creature: You gain 2 life.",None),2);
    let source=g.put(0,2,Zone::Battlefield);
    g.act(0,Move::Activate{source,ability:0,targets:vec![],paying:vec![vec![bear]]}).unwrap();pass(&mut g);
    cite!(["602.2b"]; assert_eq!(sacrificed(&g,0),vec![bear]));assert_eq!(g.players[0].life,22);assert!(g.battlefield.contains(&preserver));
    let (mut g,preserver,bear)=game(card("Offering","Instant","As an additional cost to cast this spell, sacrifice a creature.\nYou gain 3 life.",None),2);
    let spell=g.put(0,2,Zone::Hand);
    g.act(0,Move::WithCosts(vec![vec![bear]],Box::new(Move::Cast{card:spell,targets:vec![]}))).unwrap();pass(&mut g);
    cite!(["601.2h"]; assert_eq!(sacrificed(&g,0),vec![bear]));assert_eq!(g.players[0].life,23);assert_eq!(g.battlefield,vec![preserver]);
}

#[test]
fn r101_2_unsupported_cause_sources_and_actions_are_refused_without_erasing_qualifications(){
    for text in [
        "Spells your opponents control can't cause you to sacrifice permanents.",
        "Abilities your opponents control can't cause you to sacrifice permanents.",
        "Spells and abilities your opponents control can't cause you to discard cards.",
    ] {
        let d=CardDef::new("Restriction","{0}","Enchantment",text,None).unwrap();
        let reason=if text.contains("discard") {"causal prohibition: only sacrificing described permanents"}
            else {"causal prohibition: only spells and abilities qualified by controller"};
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
    }
}
