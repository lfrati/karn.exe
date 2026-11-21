//! Mandatory death-destination replacement, including simultaneous owner library ordering.
use crate::support::{cite, paid};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};
const CHOSEN: &str = "If this creature would die, put it on the bottom of its owner's library instead.";
fn card(name:&str,types:&str,text:&str,pt:Option<(i32,i32)>)->CardDef {
    let d=CardDef::new(name,"{0}",types,text,pt).unwrap();game::play::check(&d).unwrap_or_else(|e|panic!("{name}: {}",e.0));d
}
fn game(text:&str)->Game {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let chosen=CardDef::from_face(faces.iter().find(|f|f.name=="Nissa's Chosen").unwrap()).unwrap();game::play::check(&chosen).unwrap();
    let clone=CardDef::from_face(faces.iter().find(|f|f.name=="Clone").unwrap()).unwrap();game::play::check(&clone).unwrap();
    let mut g=Game::new(vec![chosen,clone,card("Effect","Instant",text,None),card("Forest","Basic Land — Forest","",None)],2,20);
    for p in 0..2 {for _ in 0..4{g.put(p,3,Zone::Library);}}g
}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
fn cast(g:&mut Game,targets:Vec<Vec<Target>>){let spell=g.put(0,2,Zone::Hand);g.act(0,Move::Cast{card:spell,targets}).unwrap();pass(g);}
fn copied(g:&mut Game,source:ObjId)->ObjId{
    let spell=g.put(0,1,Zone::Hand);g.players[0].pool.colorless=3;g.players[0].pool.colors[1]=1;
    g.act(0,paid("CCCU", Move::Cast{card:spell,targets:vec![]})).unwrap();pass(g);
    g.act(0,Move::Choose(Answer::Objects(vec![source]))).unwrap();
    *g.battlefield.iter().find(|o|g.obj(**o).card==Some(1)).unwrap()
}
#[test]
fn r614_6_full_chosen_replaces_death_with_a_single_bottom_library_move(){
    let mut g=game("Destroy target creature.");let chosen=g.put(0,0,Zone::Battlefield);let old=g.players[0].library.clone();
    cast(&mut g,vec![vec![Target::Object(chosen)]]);
    assert!(g.battlefield.is_empty());let now=g.players[0].library[0];assert_ne!(now,chosen);assert_eq!(g.obj(now).card,Some(0));
    assert_eq!(&g.players[0].library[1..],old.as_slice());
    cite!(["614.6"]; assert!(!g.log.iter().any(|e|matches!(e,Event::ZoneChanged{was,to:Zone::Graveyard,..}if *was==chosen))));
    assert_eq!(g.log.iter().filter(|e|matches!(e,Event::ZoneChanged{was,to:Zone::Library,..}if *was==chosen)).count(),1);
}
#[test]
fn r401_4_chosen_and_full_clone_order_privately_before_simultaneous_destruction_and_clone_resume(){
    let mut g=game("Destroy all creatures.");let chosen=g.put(0,0,Zone::Battlefield);let clone=copied(&mut g,chosen);let from=g.log.end();
    cast(&mut g,vec![]);
    cite!(["401.4"]; assert!(matches!(&g.decision,Some(Decision::Choose{player:0,choice:Choice::OrderCards{cards:among,..}})if among.len()==2&&among.contains(&chosen)&&among.contains(&clone)),"{:?}",g.decision));
    assert_eq!(g.collect_departed(),0,"the pending operation owns both incarnations");
    assert_eq!(g.battlefield.len(),2);assert!(!g.log.range(from..g.log.end()).iter().any(|e|matches!(e,Event::Destroyed{..})));
    let copy=g.clone();let mut logs=Vec::new();
    for (mut branch,order,expected) in [(g,vec![chosen,clone],vec![1,0]),(copy,vec![clone,chosen],vec![0,1])] {
        cite!(["401.4"]; refused("that decision is another player's"): branch.act(1,Move::Choose(Answer::Objects(order.clone()))));
        cite!(["401.4"]; refused("order every offered card exactly once (401.4, 404.3)"): branch.act(0,Move::Choose(Answer::Objects(vec![chosen,chosen]))));
        branch.act(0,Move::Choose(Answer::Objects(order))).unwrap();
        assert!(branch.battlefield.is_empty()&&branch.decision.is_none());
        cite!(["401.4"]; case: format!("{expected:?}"); assert_eq!(branch.players[0].library[..2].iter().map(|o|branch.obj(*o).card.unwrap()).collect::<Vec<_>>(),expected));
        logs.push(format!("{:?}",branch.log.range(from..branch.log.end())));
        assert!(branch.public_choices().next().is_none());
    }
    cite!(["401.4"]; assert_eq!(logs[0],logs[1],"the public event sequence must not reveal the private order"));
}
#[test]
fn r118_11_sacrifice_payment_still_succeeds_when_death_is_replaced(){
    let mut g=game("You may sacrifice a creature. If you do, you gain 3 life.");let chosen=g.put(0,0,Zone::Battlefield);
    cast(&mut g,vec![]);g.act(0,Move::Choose(Answer::Yes)).unwrap();
    assert_eq!(g.players[0].life,23);cite!(["118.11"]; assert!(g.battlefield.is_empty()));assert_eq!(g.obj(g.players[0].library[0]).card,Some(0));
    cite!(["118.11"]; assert!(g.log.iter().any(|e|matches!(e,Event::Sacrificed{object,..}if *object==chosen))));
}
fn add(g:&mut Game,def:CardDef)->usize{let defs=std::sync::Arc::make_mut(&mut g.defs);let id=defs.len();defs.push(def);id}
#[test]
fn r704_5_simultaneous_zero_toughness_and_lethal_damage_use_the_same_library_order(){
    for effect in ["All creatures get -3/-3 until end of turn.","Effect deals 3 damage to each creature."]{
        let mut g=game(effect);let chosen=g.put(0,0,Zone::Battlefield);let clone=copied(&mut g,chosen);
        cast(&mut g,vec![]);cite!(["401.4"]; case: effect.to_string(); assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::OrderCards{ref cards,..}})if cards.len()==2)));
        assert_eq!(g.battlefield.len(),2);crate::support::refused(g.act(0,Move::Pass), "a decision is pending: a choice a state-based action asks for (704.3)");let mut branch=g.clone();
        for g in [&mut g,&mut branch]{g.act(0,Move::Choose(Answer::Objects(vec![clone,chosen]))).unwrap();
            assert!(g.battlefield.is_empty()&&g.decision.is_none());assert_eq!(g.obj(g.players[0].library[0]).card,Some(0));}
    }
}
#[test]
fn r601_2h_library_order_during_a_cost_keeps_mana_life_and_prior_parts_committed_once(){
    let mut g=game("You gain 1 life.");let chosen=g.put(0,0,Zone::Battlefield);let clone=copied(&mut g,chosen);
    let def=add(&mut g,card("Altar","Artifact","{1}, Pay 2 life, Sacrifice two creatures: You gain 5 life.",None));let altar=g.put(0,def,Zone::Battlefield);
    g.players[0].pool.colorless=1;g.act(0,paid("C", Move::Activate{source:altar,ability:0,targets:vec![],paying:vec![vec![chosen,clone]]})).unwrap();
    cite!(["401.4"]; assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::OrderCards{ref cards,..}})if cards.len()==2)));
    assert_eq!(g.players[0].life,18);assert_eq!(g.players[0].pool.total(),0);assert_eq!(g.battlefield.len(),3);
    let copy=g.clone();for mut branch in [g,copy]{branch.act(0,Move::Choose(Answer::Objects(vec![clone,chosen]))).unwrap();
        cite!(["601.2h"]; assert_eq!(branch.players[0].life,18));assert_eq!(branch.battlefield,vec![altar]);pass(&mut branch);assert_eq!(branch.players[0].life,23);
        assert_eq!(branch.log.iter().filter(|e|matches!(e,Event::Sacrificed{..})).count(),2);}
}
#[test]
fn r601_2h_a_later_cost_part_rechecks_replacements_after_the_previous_part_removes_suppression(){
    let mut g=game("You gain 1 life.");let chosen=g.put(0,0,Zone::Battlefield);let clone=copied(&mut g,chosen);
    let strip=add(&mut g,card("Suppressor","Artifact","All creatures lose all abilities.",None));let suppressor=g.put(0,strip,Zone::Battlefield);
    let def=add(&mut g,card("Altar","Artifact","Sacrifice an artifact, Sacrifice two creatures: You gain 3 life.",None));let altar=g.put(0,def,Zone::Battlefield);
    g.act(0,Move::Activate{source:altar,ability:0,targets:vec![],paying:vec![vec![suppressor],vec![chosen,clone]]}).unwrap();
    cite!(["611.3b", "401.4"]; assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::OrderCards{ref cards,..}})if cards.len()==2)));
    assert!(!g.battlefield.contains(&suppressor));assert!(g.battlefield.contains(&chosen)&&g.battlefield.contains(&clone));
    let mut copy=g.clone();for g in [&mut g,&mut copy]{g.act(0,Move::Choose(Answer::Objects(vec![chosen,clone]))).unwrap();
        assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Sacrificed{object,..}if *object==suppressor)).count(),1);
        assert_eq!(g.battlefield,vec![altar]);pass(g);assert_eq!(g.players[0].life,23);}
}
#[test]
fn r614_4_simultaneous_departures_use_replacements_before_an_ability_suppressor_leaves(){
    let mut g=game("Destroy all nonland permanents.");let chosen=g.put(0,0,Zone::Battlefield);let clone=copied(&mut g,chosen);
    let strip=add(&mut g,card("Suppressor","Artifact","All creatures lose all abilities.",None));let suppressor=g.put(0,strip,Zone::Battlefield);g.battlefield.rotate_right(1);
    cast(&mut g,vec![]);
    // All three cards really enter the graveyard: suppression is read before any
    // departure. Their owner supplies the new block's order before that move commits.
    cite!(["404.3"]; assert!(matches!(&g.decision,Some(Decision::Choose{player:0,choice:Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards}})
        if cards.len()==3 && [chosen,clone,suppressor].iter().all(|card|cards.contains(card)))));
    cite!(["404.3"]; ok:g.act(0,Move::Choose(Answer::Objects(vec![chosen,clone,suppressor]))));
    assert!(g.decision.is_none()&&g.battlefield.is_empty());cite!(["614.4"]; assert_eq!(g.players[0].library.len(),4));
    for object in [chosen,clone]{assert!(g.log.iter().any(|e|matches!(e,Event::ZoneChanged{was,to:Zone::Graveyard,..}if *was==object)));}
}
#[test]
fn r614_6_replaced_death_does_not_trigger_undying_or_other_dies_abilities(){
    let mut g=game("Destroy target creature.");let text=format!("{CHOSEN}\nUndying\nWhen this creature dies, you gain 3 life.");
    let model=add(&mut g,card("Witness","Creature — Elf",&text,Some((2,3))));let object=g.put(0,model,Zone::Battlefield);
    cast(&mut g,vec![vec![Target::Object(object)]]);cite!(["614.6", "702.93a"]; assert!(g.stack.is_empty()));cite!(["614.6"]; assert_eq!(g.players[0].life,20));
    assert_eq!(g.obj(g.players[0].library[0]).card,Some(model));
}
#[test]
fn r616_1g_regeneration_replaces_destruction_before_its_contained_zone_change(){
    let mut g=game("Destroy target creature.");let chosen=g.put(0,0,Zone::Battlefield);g.regenerate(chosen);
    cast(&mut g,vec![vec![Target::Object(chosen)]]);cite!(["616.1g", "614.8"]; assert_eq!(g.battlefield,vec![chosen]));assert_eq!(g.players[0].library.len(),4);cite!(["614.8"]; assert!(g.obj(chosen).tapped));
    g.destroy_for_good(chosen);assert!(g.battlefield.is_empty());assert_eq!(g.obj(g.players[0].library[0]).card,Some(0));
}
#[test]
fn r113_6_death_replacement_does_not_replace_discard_mill_or_countering(){
    let mut g=game("You gain 1 life.");let hand=g.put(0,0,Zone::Hand);g.discard(0,hand);
    let top=g.put(0,0,Zone::Library);g.mill(0,1);
    let spell=g.put(0,0,Zone::Hand);g.players[0].pool.colors[4]=2;g.act(0,paid("GG", Move::Cast{card:spell,targets:vec![]})).unwrap();let stack=*g.stack.last().unwrap();g.counter(stack);
    for was in [hand,top,stack]{assert!(g.log.iter().any(|e|matches!(e,Event::ZoneChanged{was:o,to:Zone::Graveyard,..}if *o==was)));}
    cite!(["113.6"]; assert_eq!(g.players[0].graveyard.len(),3));
}
#[test]
fn r111_7_copied_token_reaches_the_replaced_destination_then_ceases(){
    let mut g=game("Create a token that's a copy of target creature.");let chosen=g.put(0,0,Zone::Battlefield);
    cast(&mut g,vec![vec![Target::Object(chosen)]]);let token=*g.battlefield.iter().find(|o|g.obj(**o).token()).unwrap();
    g.destroy(token);let now=g.players[0].library[0];cite!(["111.7"]; assert!(g.obj(now).token()));assert_eq!(g.def(now).unwrap().name(),"Nissa's Chosen");
    g.state_based_actions();cite!(["704.5d"]; assert_eq!(g.obj(now).zone,Zone::Gone));assert_eq!(g.players[0].library.len(),4);
    assert!(!g.log.iter().any(|e|matches!(e,Event::ZoneChanged{was,to:Zone::Graveyard,..}if *was==token)));
}
#[test]
fn r614_5_identical_instances_replace_once_and_competing_or_optional_destinations_stay_refused(){
    let d=card("Twice","Creature",&format!("{CHOSEN}\n{CHOSEN}"),Some((2,3)));let mut g=Game::new(vec![d],2,20);let object=g.put(0,0,Zone::Battlefield);g.destroy(object);
    assert_eq!(g.players[0].library.len(),1);cite!(["614.5"]; assert_eq!(g.log.iter().filter(|e|matches!(e,Event::ZoneChanged{was,..}if *was==object)).count(),1));
    for text in [format!("{CHOSEN}\nIf this creature would die, put it on top of its owner's library instead."),
        "If this creature would die, you may put it on the bottom of its owner's library instead.".into(),
        "If a creature would die, put it on the bottom of its owner's library instead.".into(),
        "If this creature would die, put it into its owner's library third from the top instead.".into()]{
        let d=CardDef::new("Unsupported","{0}","Creature",&text,Some((2,3))).unwrap();
        let reason=if text.contains('\n') {"death destination: ordering competing replacements on one object"}
            else {"zone-destination replacement: what it does instead"};
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
    }
}

#[test]
fn r608_2_sacrifice_and_zone_move_instructions_resume_their_frame_after_library_order(){
    for text in ["Sacrifice two creatures. You gain 2 life.","Put all creatures into their owners' graveyards. You gain 2 life."] {
        let mut g=game(text);let chosen=g.put(0,0,Zone::Battlefield);let clone=copied(&mut g,chosen);
        cast(&mut g,vec![]);cite!(["401.4"]; case: text.to_string(); assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::OrderCards{ref cards,..}})if cards.len()==2),"{text}: {:?}",g.decision));
        assert_eq!(g.players[0].life,20);let mut branch=g.clone();
        for g in [&mut g,&mut branch]{g.act(0,Move::Choose(Answer::Objects(vec![chosen,clone]))).unwrap();cite!(["608.2c"]; case: text.to_string(); assert_eq!(g.players[0].life,22));assert!(g.battlefield.is_empty()&&g.stack.is_empty());}
    }
}
#[test]
fn r601_2h_additional_casting_cost_order_suspends_before_cast_completion(){
    let mut g=game("As an additional cost to cast this spell, sacrifice two creatures.\nYou gain 2 life.");let chosen=g.put(0,0,Zone::Battlefield);let clone=copied(&mut g,chosen);
    let spell=g.put(0,2,Zone::Hand);g.act(0,Move::WithCosts(vec![vec![chosen,clone]],Box::new(Move::Cast{card:spell,targets:vec![]}))).unwrap();
    assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::OrderCards{ref cards,..}})if cards.len()==2));
    let source=*g.stack.last().unwrap();cite!(["601.2i"]; assert!(!g.log.iter().any(|e|matches!(e,Event::Cast{spell,..}if *spell==source))));
    g.act(0,Move::Choose(Answer::Objects(vec![clone,chosen]))).unwrap();cite!(["601.2i"]; assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Cast{spell,..}if *spell==source)).count(),1));
    pass(&mut g);assert_eq!(g.players[0].life,22);assert!(g.stack.is_empty());
}
#[test]
fn r608_2h_a_replaced_sacrifice_keeps_cost_lki_but_not_a_public_successor(){
    let mut g=game("As an additional cost to cast this spell, sacrifice a creature.\nYou gain life equal to the sacrificed creature's toughness.");
    let chosen=g.put(0,0,Zone::Battlefield);g.put_counters(chosen,"+1/+1",2);let spell=g.put(0,2,Zone::Hand);
    g.act(0,Move::WithCosts(vec![vec![chosen]],Box::new(Move::Cast{card:spell,targets:vec![]}))).unwrap();pass(&mut g);
    cite!(["608.2h"]; assert_eq!(g.players[0].life,25));cite!(["400.7"]; assert!(g.obj(g.players[0].library[0]).counters.is_empty()));
}

#[test]
fn r401_4_tokens_do_not_add_cards_to_the_library_ordering_choice(){
    let mut g=game("Create a token that's a copy of target creature.");let chosen=g.put(0,0,Zone::Battlefield);
    cast(&mut g,vec![vec![Target::Object(chosen)]]);assert_eq!(g.battlefield.len(),2);
    let sweep=add(&mut g,card("Sweep","Instant","Destroy all creatures.",None));let spell=g.put(0,sweep,Zone::Hand);
    g.act(0,Move::Cast{card:spell,targets:vec![]}).unwrap();pass(&mut g);
    cite!(["401.4"]; assert!(g.decision.is_none()&&g.battlefield.is_empty()));cite!(["111.7"]; assert_eq!(g.players[0].library.len(),5));
    assert_eq!(g.obj(g.players[0].library[0]).card,Some(0));
}
