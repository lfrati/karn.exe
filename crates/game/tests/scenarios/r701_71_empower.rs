//! Empower's explicit sequence is conditional creation, one current token choice,
//! then loyalty placement (701.71a), without an intervening SBA (704.4).
use crate::support::cite;
use game::{CardDef,Game,Move,ObjId,Zone};
use game::state::{Answer,Choice,Decision};

fn spell(text:&str)->CardDef {CardDef::new("Empowerer","{0}","Instant",text,None).unwrap()}
fn walker(subtype:&str)->CardDef {CardDef::planeswalker("Model","{0}",&format!("Planeswalker — {subtype}"),"+1: You gain 1 life.",5).unwrap()}
fn game(mut defs:Vec<CardDef>)->Game {
    defs.push(CardDef::new("Forest","","Basic Land — Forest","",None).unwrap());
    for d in &defs {game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}
    Game::new(defs,2,20)
}
fn resolve(g:&mut Game) {for _ in 0..g.players.len() {g.act(g.priority,Move::Pass).unwrap();}}
fn cast(g:&mut Game,index:usize,spending:&str) {let card=g.put(0,index,Zone::Hand);g.act(0,crate::support::paid(spending, Move::Cast{card,targets:vec![]})).unwrap();resolve(g);}
fn tokens(g:&Game)->Vec<ObjId> {g.battlefield.iter().copied().filter(|id|g.obj(*id).token()).collect()}
fn loyalty(g:&Game,id:ObjId)->u32 {g.obj(id).counters.get("loyalty").copied().unwrap_or(0)}
fn model_token(g:&mut Game,index:usize,controller:usize)->ObjId {
    let model=g.put(controller,index,Zone::Battlefield);
    let chars=g.obj(model).chars.clone();
    let token=g.create_token(controller,chars,false);g.put_counters(token,"loyalty",5);token
}

#[test]
fn r701_71a_rule_token_and_unbounded_literal_amount() {
    let mut g=game(vec![spell("Empower Jace 17.")]);cast(&mut g,0,"");
    let token=tokens(&g)[0];
    cite!(["701.71a","704.4"];assert_eq!(loyalty(&g,token),17));
    assert_eq!(g.obj(token).chars.name,"Jace Token");
    assert_eq!(g.obj(token).chars.colors,vec![ir::Color::Blue]);
    assert_eq!(g.obj(token).chars.types,vec![ir::CardType::Planeswalker]);
    assert!(g.obj(token).chars.subtypes.iter().any(|s|*s==ir::Subtype::from_name("Jace").unwrap()));
    assert!(!g.obj(token).chars.supertypes.contains(&ir::Supertype::Legendary));
    assert_eq!(g.abilities(token).len(),2);
}

#[test]
fn r701_71a_existing_token_is_required_and_only_one_is_chosen() {
    let mut g=game(vec![spell("Empower Jace 3."),walker("Jace")]);
    let a=model_token(&mut g,1,0);let b=model_token(&mut g,1,0);
    let models:Vec<_>=g.battlefield.iter().copied().filter(|o|!g.obj(*o).token()).collect();
    let spell=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card:spell,targets:vec![]}).unwrap();
    // The two model cards have zero loyalty; the distinct tokens already have five.
    cite!(["704.5i","404.3"]; ok: g.act(0,Move::Choose(Answer::Objects(models))));
    resolve(&mut g);
    let Some(Decision::Choose{player:0,choice:Choice::Objects{among,count:1,..}})=g.decision.clone() else {panic!("{:?}",g.decision)};
    cite!(["701.71a"];assert_eq!(among,vec![a,b]));
    let mut other=g.clone();
    g.act(0,Move::Choose(Answer::Objects(vec![b]))).unwrap();
    other.act(0,Move::Choose(Answer::Objects(vec![a]))).unwrap();
    assert_eq!((loyalty(&g,a),loyalty(&g,b)),(5,8));
    assert_eq!((loyalty(&other,a),loyalty(&other,b)),(8,5));
    assert_eq!(tokens(&g).len(),2);
}

#[test]
fn r701_71a_nontoken_other_subtype_and_other_controller_are_ineligible() {
    let mut g=game(vec![spell("Empower Jace 2."),walker("Jace"),walker("Garruk")]);
    let normal=g.put(0,1,Zone::Battlefield);g.put_counters(normal,"loyalty",5);
    let foreign=model_token(&mut g,1,1);let wrong=model_token(&mut g,2,0);
    cast(&mut g,0,"");
    let made=tokens(&g).into_iter().find(|id|*id!=foreign && *id!=wrong).unwrap();
    cite!(["701.71a"];assert_eq!(loyalty(&g,made),2));
    assert_eq!((loyalty(&g,normal),loyalty(&g,foreign),loyalty(&g,wrong)),(5,5,5));
}

#[test]
fn r701_71a_zero_still_creates_before_sba_and_following_instruction() {
    let mut g=game(vec![spell("Empower Jace 0. You gain 1 life.")]);cast(&mut g,0,"");
    cite!(["701.71a","107.1b","704.5i"];assert!(tokens(&g).is_empty()));
    assert_eq!(g.log.iter().filter(|e|matches!(e,game::Event::Created{..})).count(),1);
    assert_eq!(g.players[0].life,21);
}

#[test]
fn r701_71a_creation_multiplier_does_not_empower_each_result() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let catalog=CardDef::catalog(&faces);
    let doubling=CardDef::from_faces(&catalog["Doubling Season"]).unwrap();
    let mut g=game(vec![spell("Empower Jace 3."),doubling]);g.put(0,1,Zone::Battlefield);
    cast(&mut g,0,"");
    let Some(Decision::Choose{player:0,choice:Choice::Objects{among,count:1,..}})=g.decision.clone() else {panic!("{:?}",g.decision)};
    assert_eq!(among.len(),2);let chosen=among[1];
    g.act(0,Move::Choose(Answer::Objects(vec![chosen]))).unwrap();
    cite!(["701.71a","614.6","704.5i"];assert_eq!(tokens(&g),vec![chosen]));
    assert_eq!(loyalty(&g,chosen),6);
}

#[test]
fn r701_71a_counter_prohibition_does_not_skip_creation_or_later_effects() {
    let ban=CardDef::new("Ban","{0}","Enchantment","Permanents can't have counters put on them.",None).unwrap();
    let mut g=game(vec![spell("Empower Jace 4. You gain 2 life."),ban]);g.put(0,1,Zone::Battlefield);
    cast(&mut g,0,"");
    cite!(["609.3","701.71a","704.5i"];assert!(tokens(&g).is_empty()));
    assert_eq!(g.players[0].life,22);
    assert_eq!(g.log.iter().filter(|e|matches!(e,game::Event::Created{..})).count(),1);
}

#[test]
fn r701_71a_dynamic_current_quantity_and_following_instruction() {
    let mut g=game(vec![spell("Empower Jace X, where X is the number of planeswalkers you control. You gain 1 life.")]);
    cast(&mut g,0,"");
    // 608.2h at the final placement: the just-created token is now a planeswalker.
    // An early pre-creation snapshot would produce zero and lose it to SBA.
    cite!(["701.71a","608.2h"];assert_eq!(tokens(&g).len(),1));
    assert_eq!(loyalty(&g,tokens(&g)[0]),1);assert_eq!(g.players[0].life,21);
}

#[test]
fn r701_71a_printed_loyalty_abilities_resolve_and_share_turn_quota() {
    for ability in [0,1] {
        let mut g=game(vec![spell("Empower Jace 5.")]);cast(&mut g,0,"");
        let token=tokens(&g)[0];let top=g.put(0,1,Zone::Library);
        g.act(0,Move::Activate{source:token,ability,targets:vec![],paying:vec![]}).unwrap();resolve(&mut g);
        if ability==0 {
            let Some(Decision::Choose{player:0,choice:Choice::Arrange{verb:ir::Verb::Surveil,cards}})=g.decision.clone() else {panic!("{:?}",g.decision)};
            assert_eq!(cards,vec![top]);g.act(0,Move::Choose(Answer::Arrange{top:vec![],away:vec![top]})).unwrap();
            cite!(["701.71a","701.25a"];assert!(g.players[0].graveyard.iter().any(|id|g.obj(*id).card==Some(1))));
            assert_eq!(loyalty(&g,token),4);
        } else {
            cite!(["701.71a","121.1"];assert!(g.players[0].hand.iter().any(|id|g.obj(*id).card==Some(1))));
            assert_eq!(loyalty(&g,token),2);
        }
        cite!(["606.3"];assert!(g.act(0,Move::Activate{source:token,ability:0,targets:vec![],paying:vec![]}).unwrap_err().contains("already been activated")));
    }
}

#[test]
fn r608_2b_illegal_sole_target_prevents_native_no_admittance_empower() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let catalog=CardDef::catalog(&faces);
    let native=CardDef::from_faces(&catalog["No Admittance"]).unwrap();
    let bear=CardDef::new("Bear","{0}","Creature — Bear","",Some((2,2))).unwrap();
    let mut g=game(vec![native,bear]);g.players[0].pool.colors=[10;5];
    let target=g.put(1,1,Zone::Battlefield);let card=g.put(0,0,Zone::Hand);
    g.act(0,crate::support::paid("WR", Move::Cast{card,targets:vec![vec![game::Target::Object(target)]]})).unwrap();
    g.move_object(target,Zone::Hand);resolve(&mut g);
    cite!(["608.2b","701.71a"];assert!(tokens(&g).is_empty()));
    assert_eq!(g.log.iter().filter(|e|matches!(e,game::Event::Created{..})).count(),0);
}

#[test]
fn r608_2i_native_overwrite_uses_completed_exile_amount() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let catalog=CardDef::catalog(&faces);
    let native=CardDef::from_faces(&catalog["Overwrite the Multiverse"]).unwrap();
    let bear=CardDef::new("Bear","{0}","Creature — Bear","",Some((2,2))).unwrap();
    let mut g=game(vec![native,bear]);g.players[0].pool.colors=[20;5];
    g.put(0,1,Zone::Battlefield);g.put(1,1,Zone::Battlefield);
    cast(&mut g,0,"WWWWBB");
    cite!(["608.2i","701.71a"];assert_eq!(loyalty(&g,tokens(&g)[0]),2));
    assert_eq!(g.battlefield.len(),1);
}

#[test]
fn r701_71a_zero_still_requires_one_existing_candidate_choice() {
    let mut g=game(vec![spell("Empower Jace 0."),walker("Jace")]);
    let a=model_token(&mut g,1,0);let b=model_token(&mut g,1,0);let models:Vec<_>=g.battlefield.iter().copied().filter(|o|!g.obj(*o).token()).collect();
    let spell=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card:spell,targets:vec![]}).unwrap();
    // The two model cards have zero loyalty; the distinct tokens already have five.
    cite!(["704.5i","404.3"]; ok: g.act(0,Move::Choose(Answer::Objects(models))));
    resolve(&mut g);
    let Some(Decision::Choose{player:0,choice:Choice::Objects{among,count:1,..}})=g.decision.clone() else {panic!("{:?}",g.decision)};
    cite!(["701.71a","107.1b"];assert_eq!(among,vec![a,b]));
    crate::support::refused(unchanged(&mut g, |g| g.act(0, Move::Choose(Answer::Objects(vec![])))), "choose exactly 1 of them to choose (608.2d)");
    g.act(0,Move::Choose(Answer::Objects(vec![a]))).unwrap();
    assert_eq!((loyalty(&g,a),loyalty(&g,b)),(5,5));assert_eq!(tokens(&g).len(),2);
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
