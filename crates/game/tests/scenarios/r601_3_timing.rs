//! Restrictions prevail over flash and resolution permissions (101.2, 307.5, 601.3).
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};
fn game() -> Game {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let full=|name| CardDef::from_face(faces.iter().find(|f|f.face_name==name).unwrap()).unwrap();
    let defs=vec![full("Teferi, Time Raveler"),
        CardDef::new("Instant","{0}","Instant","You gain 1 life.",None).unwrap(),
        CardDef::new("Sorcery","{0}","Sorcery","You gain 1 life.",None).unwrap(),
        CardDef::new("Flash Bear","{0}","Creature — Bear","Flash",Some((2,2))).unwrap(),
        CardDef::new("Creature","{0}","Creature — Bear","",Some((2,2))).unwrap(),
        CardDef::new("Forest","","Basic Land — Forest","",None).unwrap(),
        full("Emet-Selch of the Third Seat"),
        CardDef::new("Loss","{0}","Instant","Each opponent loses 1 life.",None).unwrap(),
        CardDef::new("Grant","{0}","Instant","You may cast sorcery spells this turn as though they had flash.",None).unwrap()];
    for d in &defs {game::play::check(d).unwrap_or_else(|e|panic!("{}: {e:?}",d.name()));}
    let mut g=Game::new(defs,3,20);
    for p in 0..3 { for _ in 0..12 {g.put(p,5,Zone::Library);} }
    g
}
fn teferi(g:&mut Game,p:usize)->ObjId {let o=g.put(p,0,Zone::Battlefield);g.set_initial_counters(o,"loyalty",4);o}
fn pass(g:&mut Game){for _ in 0..3{g.act(g.priority,Move::Pass).unwrap();}}

fn activate(g:&mut Game,o:ObjId,ability:usize,targets:Vec<Vec<Target>>){g.act(0,Move::Activate{source:o,ability,targets,paying:vec![]}).unwrap();}
fn next_main(g:&mut Game,p:usize){
    let turn=g.turn;
    for _ in 0..200 {if g.turn>turn && g.active==p && g.part.main() && g.priority==p {return;}g.act(g.priority,Move::Pass).unwrap();}
    panic!("did not reach next main");
}
#[test]
fn full_card_restriction_uses_player_turn_priority_and_empty_stack(){
    let mut g=game();teferi(&mut g,1);
    let instant=g.put(0,1,Zone::Hand);let flash=g.put(0,3,Zone::Hand);
    assert!(g.clone().act(g.priority, Move::Cast { card: instant, targets: vec![] }).is_ok());assert!(g.clone().act(g.priority, Move::Cast { card: flash, targets: vec![] }).is_ok());
    g.act(0,Move::Cast{card:instant,targets:vec![]}).unwrap();
    let another=g.put(0,1,Zone::Hand);
    cite!(["307.5"]; assert!(!g.clone().act(g.priority, Move::Cast { card: another, targets: vec![] }).is_ok()));cite!(["307.5","101.2"]; assert!(!g.clone().act(g.priority, Move::Cast { card: flash, targets: vec![] }).is_ok()));
    cite!(["307.5"]; refused("an effect permits casting spells only at sorcery timing (307.5, 601.3)"): g.act(0,Move::Cast{card:another,targets:vec![]}));
    cite!(["307.5"]; refused("an effect permits casting spells only at sorcery timing (307.5, 601.3)"): g.act(0,Move::Cast{card:flash,targets:vec![]}));
    pass(&mut g);cite!(["307.5"]; assert!(g.clone().act(g.priority, Move::Cast { card: another, targets: vec![] }).is_ok()));
    next_main(&mut g,1);g.act(1,Move::Pass).unwrap();assert_eq!(g.priority,2);g.act(2,Move::Pass).unwrap();
    cite!(["307.5"]; assert!(!g.clone().act(g.priority, Move::Cast { card: another, targets: vec![] }).is_ok()));cite!(["307.5"]; refused("an effect permits casting spells only at sorcery timing (307.5, 601.3)"): g.act(0,Move::Cast{card:another,targets:vec![]}));
}
#[test]
fn source_leaving_ends_restriction_immediately(){
    let mut g=game();let source=teferi(&mut g,1);let top=g.put(0,1,Zone::Hand);
    g.act(0,Move::Cast{card:top,targets:vec![]}).unwrap();let card=g.put(0,3,Zone::Hand);assert!(!g.clone().act(g.priority, Move::Cast { card: card, targets: vec![] }).is_ok());
    g.move_object(source,Zone::Hand);cite!(["611.3b"]; assert!(g.clone().act(g.priority, Move::Cast { card: card, targets: vec![] }).is_ok()));g.act(0,Move::Cast{card,targets:vec![]}).unwrap();
}
#[test]
fn resolved_permission_survives_source_and_other_turns_but_expires_at_controller_turn(){
    let mut g=game();let source=teferi(&mut g,0);activate(&mut g,source,1,vec![]);pass(&mut g);
    assert!(g.decision.is_none());
    g.move_object(source,Zone::Hand);next_main(&mut g,1);
    g.act(1,Move::Pass).unwrap();g.act(2,Move::Pass).unwrap();
    let sorcery=g.put(0,2,Zone::Hand);let creature=g.put(0,4,Zone::Hand);
    cite!(["611.2a"]; assert!(g.clone().act(g.priority, Move::Cast { card: sorcery, targets: vec![] }).is_ok()));cite!(["117.1a"]; assert!(!g.clone().act(g.priority, Move::Cast { card: creature, targets: vec![] }).is_ok()));
    let mut cloned=g.clone();cloned.act(0,Move::Cast{card:sorcery,targets:vec![]}).unwrap();
    next_main(&mut g,2);
    next_main(&mut g,0);
    let instant=g.put(0,1,Zone::Hand);g.act(0,Move::Cast{card:instant,targets:vec![]}).unwrap();
    cite!(["611.2a"]; assert!(!g.clone().act(g.priority, Move::Cast { card: sorcery, targets: vec![] }).is_ok()));cite!(["117.1a"]; refused("that is cast in a main phase of your own turn with the stack empty (117.1a)"): g.act(0,Move::Cast{card:sorcery,targets:vec![]}));
}
#[test]
fn restriction_prevails_over_resolved_flash_permission(){
    let mut g=game();let source=teferi(&mut g,0);activate(&mut g,source,1,vec![]);pass(&mut g);teferi(&mut g,1);
    let instant=g.put(0,1,Zone::Hand);g.act(0,Move::Cast{card:instant,targets:vec![]}).unwrap();
    let sorcery=g.put(0,2,Zone::Hand);cite!(["307.5","101.2"]; assert!(!g.clone().act(g.priority, Move::Cast { card: sorcery, targets: vec![] }).is_ok()));cite!(["307.5"]; refused("an effect permits casting spells only at sorcery timing (307.5, 601.3)"): g.act(0,Move::Cast{card:sorcery,targets:vec![]}));
}
#[test]
fn this_turn_permission_is_installed_without_optional_action_and_cleans_up(){
    let mut g=game();let grant=g.put(0,8,Zone::Hand);g.act(0,Move::Cast{card:grant,targets:vec![]}).unwrap();pass(&mut g);
    assert!(g.decision.is_none());next_main(&mut g,1);
    g.act(1,Move::Pass).unwrap();g.act(2,Move::Pass).unwrap();
    let sorcery=g.put(0,2,Zone::Hand);cite!(["611.2a"]; assert!(!g.clone().act(g.priority, Move::Cast { card: sorcery, targets: vec![] }).is_ok()));
}
#[test]
fn zero_target_minus_three_draws_but_illegal_chosen_target_fizzles(){
    let mut g=game();let source=teferi(&mut g,0);let hand=g.players[0].hand.len();
    activate(&mut g,source,2,vec![vec![]]);pass(&mut g);cite!(["601.2c","602.2b"]; assert_eq!(g.players[0].hand.len(),hand+1));
    cite!(["606.4"]; assert_eq!(g.obj(source).counters.get("loyalty"),Some(&1)));
    let mut g=game();let source=teferi(&mut g,0);let target=g.put(1,4,Zone::Battlefield);let hand=g.players[0].hand.len();
    activate(&mut g,source,2,vec![vec![Target::Object(target)]]);g.move_object(target,Zone::Graveyard);pass(&mut g);
    cite!(["608.2b"]; assert_eq!(g.players[0].hand.len(),hand));
}
#[test]
fn minus_three_returns_to_owner_and_draws(){
    let mut g=game();let source=teferi(&mut g,0);let target=g.put(1,4,Zone::Battlefield);let hand=g.players[0].hand.len();
    activate(&mut g,source,2,vec![vec![Target::Object(target)]]);pass(&mut g);
    cite!(["400.3"]; assert!(g.players[1].hand.iter().any(|o|g.obj(*o).card==Some(4))));assert_eq!(g.players[0].hand.len(),hand+1);
}
#[test]
fn emet_cast_offer_cannot_override_restriction_or_consume_allowance_on_rejection(){
    let mut g=game();g.put(0,6,Zone::Battlefield);let restricted=teferi(&mut g,1);g.put(0,2,Zone::Graveyard);
    let loss=g.put(0,7,Zone::Hand);g.act(0,Move::Cast{card:loss,targets:vec![]}).unwrap();pass(&mut g);
    let card=g.players[0].graveyard.iter().find(|o|g.obj(**o).card==Some(2)).copied().unwrap();
    g.act(0,Move::ChooseTargets(vec![vec![Target::Object(card)]])).unwrap();pass(&mut g);
    assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Cast{..}})));
    cite!(["307.5"]; refused("an effect permits casting spells only at sorcery timing (307.5, 601.3)"): g.act(0,Move::Cast{card,targets:vec![]}));
    assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Cast{..}})));
    g.move_object(restricted,Zone::Hand);let mut clone=g.clone();
    cite!(["611.3b","601.3"]; ok: clone.act(0,Move::Cast{card,targets:vec![]}));pass(&mut clone);assert_eq!(clone.players[0].life,21);
    g.act(0,Move::Choose(Answer::No)).unwrap();
}
#[test]
fn permission_matches_selected_adventure_face_and_static_permission_is_live(){
    let adventure=CardDef::adventurer(
        CardDef::new("Adventurer","{0}","Creature — Bear","",Some((2,2))).unwrap(),
        CardDef::new("Journey","{0}","Sorcery — Adventure","You gain 1 life.",None).unwrap()).unwrap();
    // Keep the same compiled program in a fixture with the alternative definition present.
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let mut g=Game::new(vec![CardDef::from_face(faces.iter().find(|f|f.face_name=="Teferi, Time Raveler").unwrap()).unwrap(),adventure],3,20);
    let source=teferi(&mut g,0);activate(&mut g,source,1,vec![]);pass(&mut g);
    let card=g.put(0,1,Zone::Hand);g.active=1;g.priority=0;
    cite!(["601.3b", "715.3a"]; refused("cast in a main phase of your own turn with the stack empty"): act_unchanged(&mut g, 0, Move::Cast { card, targets: vec![] }));
    g.act(0,Move::CastAs(game::card::CastAs::Adventure,Box::new(Move::Cast{card,targets:vec![]}))).unwrap();
    pass(&mut g);
    assert_eq!(g.players[0].life,21);
    assert_eq!(g.exile.len(),1);

    let grant=CardDef::new("Granting Enchantment","{0}","Enchantment","You may cast sorcery spells as though they had flash.",None).unwrap();
    game::play::check(&grant).unwrap();
    let spell=CardDef::new("Life Spell","{0}","Sorcery","You gain 1 life.",None).unwrap();
    let mut g=Game::new(vec![grant,spell],2,20);let source=g.put(0,0,Zone::Battlefield);let card=g.put(0,1,Zone::Hand);
    g.active=1;
    let mut permitted=g.clone();
    cite!(["611.3b"]; ok: permitted.act(0,Move::Cast{card,targets:vec![]}));
    for _ in 0..2 {permitted.act(permitted.priority,Move::Pass).unwrap();}
    assert_eq!(permitted.players[0].life,21);
    g.move_object(source,Zone::Graveyard);
    cite!(["611.3b","117.1a"]; refused("cast in a main phase of your own turn with the stack empty"): act_unchanged(&mut g,0,Move::Cast{card,targets:vec![]}));
}
#[test]
fn casting_timing_rules_require_a_player_subject_and_bounded_descriptions(){
    for text in ["Creatures can cast spells only any time they could cast a sorcery.",
                 "Creatures may cast sorcery spells as though they had flash."] {
        assert_eq!(CardDef::new("Bad Rule","{0}","Enchantment",text,None).expect_err("casting actor must be a player"),"text the grammar does not read","{text}");
    }
    let card=CardDef::new("Unsupported","{0}","Sorcery","Until your next turn, you may cast target sorcery spell as though it had flash.",None).unwrap();
    crate::support::refused(game::play::check(&card).map_err(|e|e.0),"flash permission: only described spells");
}

#[track_caller]
fn act_unchanged(g:&mut Game, player:usize, action:Move)->Result<(),String> {
 let before=format!("{g:?}");let result=g.act(player,action);assert_eq!(format!("{g:?}"),before);result
}
