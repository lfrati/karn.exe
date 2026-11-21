//! Modal faces are selected before casting/land legality (712.11–14).
use crate::support::{cite, paid};
use game::{CardDef,Game,Move,Zone,ObjId,Target};
use game::card::CastAs;
use game::state::{Answer,Decision,Choice};
use ir::{CardType,Color};
fn sink()->CardDef {let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let catalog=CardDef::catalog(&faces);CardDef::from_faces(&catalog["Sink into Stupor"]).unwrap()}
fn setup()->Game {let d=sink();game::play::check(&d).unwrap();Game::new(vec![d,
    CardDef::new("Bear","{0}","Creature — Bear","",Some((2,2))).unwrap(),
    CardDef::new("Land","","Basic Land — Island","",None).unwrap(),
    CardDef::new("Spell","{0}","Instant","You gain 1 life.",None).unwrap()],3,20)}
fn pass(g:&mut Game){for _ in 0..3{g.act(g.priority,Move::Pass).unwrap();}}
fn land(g:&mut Game,card:ObjId){g.act(0,Move::PlayLandAs{card,face:1}).unwrap();}
#[test]
fn r712_11_12_full_sink_offers_spell_and_land_faces_with_distinct_legality() {
    let mut g=setup();let card=g.put(0,0,Zone::Hand);let enemy=g.put(1,1,Zone::Battlefield);g.players[0].pool.colors[Color::Blue as usize]=3;
    for (face,why) in [(0,"that is not a land"),(2,"choose front face 0 or back face 1 (712.11–12)")] {cite!(["712.12"]; case: why; refused(why): g.act(0,Move::PlayLandAs{card,face}));}
    cite!(["712.12"]; refused("that is not a land"): g.act(0,Move::PlayLand(card)));
    cite!(["712.11c", "305.9"]; refused("a land is played, not cast (305.9)"): g.act(0,Move::CastAs(CastAs::Face(1),Box::new(Move::Cast{card,targets:vec![]}))));
    g.act(0,paid("UUU", Move::CastAs(CastAs::Face(0),Box::new(Move::Cast{card,targets:vec![vec![Target::Object(enemy)]]})))).unwrap();
    assert_eq!(g.def(g.stack[0]).unwrap().name(),"Sink into Stupor");assert_eq!(g.obj(g.stack[0]).chars.mana_value,3);
    pass(&mut g);assert_eq!(g.players[1].hand.len(),1);assert_eq!(g.players[0].graveyard.len(),1);assert!(g.def(g.players[0].graveyard[0]).unwrap().is(CardType::Instant));
}
#[test]
fn r614_12_soporific_optional_life_entry_suspends_clones_and_consumes_land_play_once() {
    for pay in [false,true] {
        let mut g=setup();let card=g.put(0,0,Zone::Hand);land(&mut g,card);
        cite!(["614.1c"]; assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::May}))));
        assert!(g.players[0].hand.contains(&card));assert_eq!(g.def(card).unwrap().name(),"Sink into Stupor");
        crate::support::refused(g.act(1,Move::Choose(Answer::No)), "that decision is another player's");crate::support::refused(g.act(0,Move::Choose(Answer::Objects(vec![]))), "the choice is whether to do it: yes or no (608.2d)");
        assert!(g.players[0].hand.contains(&card));assert!(!g.players[0].land_played);
        let mut clone=g.clone();
        for branch in [&mut g,&mut clone] {
            branch.act(0,Move::Choose(if pay {Answer::Yes}else{Answer::No})).unwrap();let permanent=*branch.battlefield.last().unwrap();
            cite!(["712.12"]; assert_eq!(branch.def(permanent).unwrap().name(),"Soporific Springs"));assert_eq!(branch.obj(permanent).chars.mana_value,0);
            cite!(["614.1c"]; case: pay; assert_eq!(branch.obj(permanent).tapped,!pay));assert_eq!(branch.players[0].life,if pay {17}else{20});assert!(branch.players[0].land_played);
            let second=branch.put(0,0,Zone::Hand);cite!(["305.2"]; refused("you have played a land this turn (305.2)"): branch.act(0,Move::PlayLandAs{card:second,face:1}));
            if pay {branch.act(0,Move::Activate{source:permanent,ability:1,targets:vec![],paying:vec![]}).unwrap();assert_eq!(branch.players[0].pool.colors[Color::Blue as usize],1);}
        }
    }
}
#[test]
fn r712_14b_generic_entry_cannot_substitute_land_back_for_nonpermanent_front() {
    for zone in [Zone::Hand,Zone::Library,Zone::Graveyard,Zone::Exile] {
        let mut g=setup();let card=g.put(0,0,zone);g.move_object(card,Zone::Battlefield);assert_eq!(g.obj(card).zone,zone);cite!(["712.14b"]; assert!(g.battlefield.is_empty()));
    }
}
#[test]
fn r712_8a_zone_changes_restore_front_and_modal_land_cannot_transform_to_instant() {
    for zone in [Zone::Hand,Zone::Library,Zone::Graveyard,Zone::Exile] {
        let mut g=setup();let card=g.put(0,0,Zone::Hand);land(&mut g,card);g.act(0,Move::Choose(Answer::No)).unwrap();let permanent=*g.battlefield.last().unwrap();
        cite!(["712.10"]; case: format!("{zone:?}"); assert!(!g.transform(permanent,None)));assert_eq!(g.def(permanent).unwrap().name(),"Soporific Springs");
        g.move_object(permanent,zone);let card=match zone {Zone::Hand=>g.players[0].hand[0],Zone::Library=>g.players[0].library[0],Zone::Graveyard=>g.players[0].graveyard[0],_=>g.exile[0]};
        cite!(["712.8a"]; case: format!("{zone:?}"); assert_eq!(g.def(card).unwrap().name(),"Sink into Stupor"));assert!(g.def(card).unwrap().is(CardType::Instant));assert!(!g.obj(card).transformed);assert_eq!(g.obj(card).chars.mana_value,3);
    }
}

fn modal(front: (&str,&str,&str), back: (&str,&str,&str))->CardDef {
    CardDef::modal(CardDef::new("Front",front.0,front.1,front.2,Some((2,2))).unwrap(),
        CardDef::new("Back",back.0,back.1,back.2,Some((4,4))).unwrap()).unwrap()
}
#[test]
fn r712_11_selected_back_spell_uses_own_cost_timing_value_and_entry_face() {
    let model=modal(("{G}","Sorcery","You gain 1 life."),("{3}{U}","Creature — Bird","Flash"));
    assert!(model.color_identity().contains(&Color::Green));assert!(model.color_identity().contains(&Color::Blue));
    let mut g=Game::new(vec![model],3,20);let card=g.put(0,0,Zone::Hand);
    g.act(0,Move::Pass).unwrap();g.act(1,Move::Pass).unwrap();g.act(2,Move::Pass).unwrap();
    // Outside the main phase, only the back's flash permits casting.
    assert!(!g.part.main());g.players[0].pool.colors[Color::Blue as usize]=4;g.players[0].pool.colors[Color::Green as usize]=1;
    cite!(["117.1a"]; refused("that is cast in a main phase of your own turn with the stack empty (117.1a)"): g.act(0,paid("G", Move::CastAs(CastAs::Face(0),Box::new(Move::Cast{card,targets:vec![]})))));
    g.act(0,paid("UUUU", Move::CastAs(CastAs::Face(1),Box::new(Move::Cast{card,targets:vec![]})))).unwrap();
    let spell=g.stack[0];assert_eq!(g.def(spell).unwrap().name(),"Back");cite!(["712.8f"]; assert_eq!(g.obj(spell).chars.mana_value,4));
    cite!(["712.11b"]; assert_eq!(g.players[0].pool.colors[Color::Blue as usize],0));assert_eq!(g.players[0].pool.colors[Color::Green as usize],1);
    pass(&mut g);let creature=g.battlefield[0];cite!(["712.13"]; assert_eq!(g.def(creature).unwrap().name(),"Back"));assert_eq!(g.obj(creature).chars.mana_value,4);assert_eq!(g.power(creature),Some(4));
    g.move_object(creature,Zone::Hand);let card=g.players[0].hand[0];cite!(["712.8a"]; assert_eq!(g.def(card).unwrap().name(),"Front"));assert_eq!(g.obj(card).chars.mana_value,1);
}
#[test]
fn r707_8a_modal_back_copies_keep_own_mana_value_and_both_faces() {
    let model=modal(("{G}","Creature — Bear",""),("{3}{U}","Creature — Bird",""));
    let copy=CardDef::new("Copy","{0}","Instant","Create a token that's a copy of target creature.",None).unwrap();
    let clone=CardDef::new("Clone","{0}","Creature","You may have this creature enter as a copy of any creature on the battlefield.",Some((0,0))).unwrap();
    let mut g=Game::new(vec![model,copy,clone],3,20);let source=g.put(0,0,Zone::Battlefield);assert!(g.transform(source,None));
    let card=g.put(0,1,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![vec![Target::Object(source)]]}).unwrap();pass(&mut g);
    let token=*g.battlefield.last().unwrap();assert!(g.obj(token).token());cite!(["707.8", "712.8f"]; assert_eq!(g.obj(token).chars.mana_value,4));cite!(["707.8"]; assert_eq!(g.def(token).unwrap().name(),"Back"));
    // A second generation retains the same printed back value.
    let card=g.put(0,1,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![vec![Target::Object(token)]]}).unwrap();pass(&mut g);
    let second=*g.battlefield.last().unwrap();assert_eq!(g.obj(second).chars.mana_value,4);cite!(["707.8a"]; assert!(g.transform(second,None)));cite!(["707.8a"]; assert_eq!(g.obj(second).chars.mana_value,1));
    let card=g.put(0,2,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![]}).unwrap();pass(&mut g);g.act(0,Move::Choose(Answer::Objects(vec![source]))).unwrap();
    let clone=*g.battlefield.last().unwrap();assert_eq!(g.obj(clone).chars.mana_value,4);assert_eq!(g.def(clone).unwrap().name(),"Back");assert!(!g.transform(clone,None));
}
#[test]
fn r712_12_two_land_faces_and_default_entry_front_share_one_land_limit() {
    let model=modal(("","Land","{T}: Add {G}."),("","Land","{T}: Add {U}."));
    for face in [0,1] {
        let mut g=Game::new(vec![model.clone()],3,20);let card=g.put(0,0,Zone::Hand);
        for chosen in [0,1] {cite!(["712.12"]; case: chosen; ok: g.clone().act(0, Move::PlayLandAs { card, face: chosen }));}
        g.act(0,Move::PlayLandAs{card,face}).unwrap();let land=g.battlefield[0];cite!(["712.12"]; case: face; assert_eq!(g.def(land).unwrap().name(),if face==0 {"Front"}else{"Back"}));
        let other=g.put(0,0,Zone::Hand);for chosen in [0,1]{cite!(["305.2"]; case: chosen; refused("you have played a land this turn (305.2)"): g.act(0,Move::PlayLandAs{card:other,face:chosen}));}
        g.act(0,Move::Activate{source:land,ability:0,targets:vec![],paying:vec![]}).unwrap();assert_eq!(g.players[0].pool.colors[if face==0 {Color::Green}else{Color::Blue} as usize],1);
    }
    let mut g=Game::new(vec![model],3,20);let card=g.put(0,0,Zone::Hand);g.move_object(card,Zone::Battlefield);cite!(["712.14"]; assert_eq!(g.def(g.battlefield[0]).unwrap().name(),"Front"));
}
#[test]
fn r614_12_impossible_life_payment_enters_tapped_without_an_offer() {
    let mut g=setup();g.players[0].life=2;let card=g.put(0,0,Zone::Hand);land(&mut g,card);
    assert!(g.decision.is_none());crate::support::refused(g.act(0,Move::Choose(Answer::Yes)), "no decision is pending");
    assert!(!g.players[0].hand.contains(&card));assert!(g.players[0].land_played);
    cite!(["118.3", "614.1c"]; assert!(g.obj(g.battlefield[0]).tapped));assert_eq!(g.players[0].life,2);
}
#[test]
fn r712_11_full_sink_targets_only_opponents_spells_or_nonland_permanents() {

    let mut g=setup();let card=g.put(0,0,Zone::Hand);let own=g.put(0,1,Zone::Battlefield);let land=g.put(1,2,Zone::Battlefield);
    let enemy=g.put(1,1,Zone::Battlefield);g.players[0].pool.colors[Color::Blue as usize]=3;
    for (targets,rule,why) in [(vec![],"601.2c","that has 1 target group(s), 0 given"),(vec![vec![Target::Object(own)]],"115.1","an illegal target, Bear: the target is one an opponent controls, and you control it (115.1)"),(vec![vec![Target::Object(land)]],"115.1","an illegal target, Land: it is a land (115.1)")] {
        cite!([rule]; case: why; refused(why): unchanged(&mut g,paid("UUU",Move::CastAs(CastAs::Face(0),Box::new(Move::Cast{card,targets})))));
        assert!(g.players[0].hand.contains(&card));assert_eq!(g.players[0].pool.colors[Color::Blue as usize],3);
    }
    g.act(0,Move::Pass).unwrap();let opponent=g.put(1,3,Zone::Hand);g.act(1,Move::Cast{card:opponent,targets:vec![]}).unwrap();g.act(1,Move::Pass).unwrap();g.act(2,Move::Pass).unwrap();
    let spell=g.stack[0];cite!(["305.1"]; refused("a land is played in a main phase of your own turn with the stack empty (305.1)"): unchanged(&mut g,Move::PlayLandAs { card, face: 1 }));
    g.act(0,paid("UUU",Move::CastAs(CastAs::Face(0),Box::new(Move::Cast{card,targets:vec![vec![Target::Object(spell)]]})))).unwrap();pass(&mut g);
    assert!(g.stack.is_empty());assert_eq!(g.players[1].hand.len(),1);assert_eq!(g.players[0].life,20);assert!(g.battlefield.contains(&enemy));
}
#[test]
fn r707_10g_copy_of_selected_modal_permanent_spell_enters_on_same_face() {
    let model=modal(("{G}","Creature — Bear",""),("{3}{U}","Creature — Bird",""));
    let fork=CardDef::new("Fork","{0}","Instant","Copy target spell.",None).unwrap();
    let mut g=Game::new(vec![model,fork],3,20);let card=g.put(0,0,Zone::Hand);g.players[0].pool.colors[Color::Blue as usize]=4;
    g.act(0,paid("UUUU", Move::CastAs(CastAs::Face(1),Box::new(Move::Cast{card,targets:vec![]})))).unwrap();let original=g.stack[0];
    let fork=g.put(0,1,Zone::Hand);g.act(0,Move::Cast{card:fork,targets:vec![vec![Target::Object(original)]]}).unwrap();pass(&mut g);
    assert_eq!(g.stack.len(),2);let copied=*g.stack.last().unwrap();cite!(["707.10g"]; assert_eq!(g.def(copied).unwrap().name(),"Back"));assert_eq!(g.obj(copied).chars.mana_value,4);
    pass(&mut g);let token=g.battlefield[0];cite!(["707.10f"]; assert!(g.obj(token).token()));cite!(["712.13"]; assert_eq!(g.def(token).unwrap().name(),"Back"));assert_eq!(g.obj(token).chars.mana_value,4);
    assert!(g.transform(token,None));cite!(["707.10g"]; assert_eq!(g.def(token).unwrap().name(),"Front"));assert_eq!(g.obj(token).chars.mana_value,1);
    pass(&mut g);let permanent=*g.battlefield.last().unwrap();assert_eq!(g.def(permanent).unwrap().name(),"Back");
}
#[test]
fn r707_12_retained_modal_card_copy_can_cast_its_back_with_back_instructions() {
    let model=modal(("{G}","Instant","You gain 1 life."),("{3}{U}","Instant","You gain 4 life."));
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let catalog=CardDef::catalog(&faces);
    let chandra=CardDef::from_faces(&catalog["Chandra, Pyromaster"]).unwrap();
    let land=CardDef::new("Land","","Basic Land — Mountain","",None).unwrap();
    let fork=CardDef::new("Fork","{0}","Instant","Copy target spell.",None).unwrap();
    for face in [0,1] {
    let mut g=Game::new(vec![model.clone(),chandra.clone(),land.clone(),fork.clone()],3,20);
    for _ in 0..12 {g.put(0,2,Zone::Library);}g.put(0,0,Zone::Library);
    let chandra=g.put(0,1,Zone::Battlefield);g.put_counters(chandra,"loyalty",7-g.obj(chandra).counters.get("loyalty").copied().unwrap_or(0));
    g.act(0,Move::Activate{source:chandra,ability:2,targets:vec![],paying:vec![]}).unwrap();pass(&mut g);
    let Some((0,game::legal::Asked::Cast{cards,..}))=g.asked() else {panic!("{:?}",g.asked())};
    g.act(0,Move::CastAs(CastAs::Face(face),Box::new(Move::Cast{card:cards[0],targets:vec![]}))).unwrap();
    g.act(0,Move::Choose(Answer::No)).unwrap();
    let spell=g.stack[0];cite!(["712.11b", "712.8f"]; case: face; assert_eq!(g.obj(spell).chars.mana_value,if face==1 {4}else{1}));cite!(["712.11b"]; case: face; assert_eq!(g.def(spell).unwrap().name(),if face==1 {"Back"}else{"Front"}));
    let fork=g.put(0,3,Zone::Hand);g.act(0,Move::Cast{card:fork,targets:vec![vec![Target::Object(spell)]]}).unwrap();pass(&mut g);
    assert_eq!(g.stack.len(),2);pass(&mut g);pass(&mut g);cite!(["707.10"]; case: face; assert_eq!(g.players[0].life,if face==1 {28}else{22}));
    }
}

#[track_caller]
fn unchanged(g: &mut Game, action: Move) -> Result<(), String> {
    let before=format!("{g:?}"); let result=g.act(0,action);
    assert_eq!(format!("{g:?}"),before); result
}
