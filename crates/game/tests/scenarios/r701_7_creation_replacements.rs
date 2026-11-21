//! Creation replaces the original specifications before individual entry (701.7b).
use crate::support::cite;
use game::{CardDef,Event,Game,Move,ObjId,Target,Zone};
use game::state::{Answer,Choice,Decision};

const MULTIPLIERS:[&str;5]=["Doubling Season","Exalted Sunborn","Ojer Taq, Deepest Foundation","Bard, King of Dale","Elspeth, Storm Slayer"];
fn native(names:&[&str])->Vec<CardDef> {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog=CardDef::catalog(&faces);
    names.iter().map(|name| {
        if matches!(*name,"Bard, King of Dale"|"Ojer Taq, Deepest Foundation"|"Elspeth, Storm Slayer") {
            // Exact native clause evidence; independent full-card blockers remain refused.
            let face=catalog[*name].iter().find(|f|f.face==0).unwrap();
            let text=face.oracle_text.lines().find(|t|t.starts_with("If one or more") && t.contains("tokens would be created")).unwrap();
            CardDef::new(&format!("{name} creation clause"),"{0}","Enchantment",text,None).unwrap()
        } else {CardDef::from_faces(&catalog[*name]).unwrap_or_else(|e|panic!("{name}: {e}"))}
    }).collect()
}
fn spell(text:&str)->CardDef {CardDef::new("Maker","{0}","Instant",text,None).unwrap()}
fn resolve(g:&mut Game) {for _ in 0..g.players.len() {g.act(g.priority,Move::Pass).unwrap();}}
fn cast(g:&mut Game,card:usize,targets:Vec<Vec<Target>>,spending:&str) {
    let card=g.put(0,card,Zone::Hand);g.act(0,crate::support::paid(spending, Move::Cast {card,targets})).unwrap();resolve(g);
}
fn choose(g:&mut Game,source:ObjId) {
    let Some(Decision::Choose {player,choice:Choice::Replacement {among,..}})=g.decision.clone() else {panic!("creation ordering: {:?}",g.decision)};
    let id=among.into_iter().find(|id|id.source==source).expect("authored source offered");
    g.act(player,Move::Choose(Answer::Replacement(id))).unwrap();
}
fn tokens(g:&Game)->Vec<ObjId> {g.battlefield.iter().copied().filter(|on|g.obj(*on).token()).collect()}

#[test]
fn r701_7b_native_multipliers_match_base_token_specifications() {
    let defs=native(&MULTIPLIERS);
    for (i,multiplier) in defs.into_iter().enumerate() {
        game::play::check(&multiplier).unwrap_or_else(|e|panic!("{}: {}",multiplier.name(),e.0));
        let mut g=Game::new(vec![multiplier,spell("Create a 1/1 white Soldier creature token and a Treasure token.")],2,20);
        g.put(0,0,Zone::Battlefield);cast(&mut g,1,vec![],"");
        let soldiers=tokens(&g).iter().filter(|on|g.obj(**on).chars.types.contains(&ir::CardType::Creature)).count();
        cite!(["701.7b","614.5"];assert_eq!((soldiers,tokens(&g).len()-soldiers),if i==2 {(3,1)}else{(2,2)}));
    }
}

#[test]
fn r616_1_creation_question_retains_complete_inventory_and_each_effect_once() {
    let mut defs=native(&["Exalted Sunborn","Ojer Taq, Deepest Foundation"]);
    defs.push(spell("Create a 1/1 white Soldier creature token and a Treasure token."));
    for first in [0,1] {
        let mut g=Game::new(defs.clone(),2,20);let sun=g.put(0,0,Zone::Battlefield);let ojer=g.put(0,1,Zone::Battlefield);
        cast(&mut g,2,vec![],"");
        let Some(Decision::Choose {player,choice:Choice::Replacement {on,tokens:Some(proposal),among,..}})=&g.decision else {panic!("exact creation inventory")};
        assert_eq!((*player,*on,proposal.by),(0,Target::Player(0),0));
        assert_eq!(proposal.inventory.iter().map(|t|(t.chars.name.as_str(),t.count)).collect::<Vec<_>>(),vec![("Soldier Token",1),("Treasure Token",1)]);
        assert_eq!(among.len(),2);assert!(tokens(&g).is_empty());
        choose(&mut g,if first==0 {sun}else{ojer});
        cite!(["616.1","614.5"];assert_eq!(tokens(&g).len(),8));
        assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Created {..})).count(),8);
    }
}

#[test]
fn r701_7_creator_is_distinct_from_spell_controller() {
    let mut defs=native(&["Doubling Season","Exalted Sunborn"]);
    defs.push(spell("Target player creates a Treasure token."));
    let mut g=Game::new(defs,2,20);g.put(0,0,Zone::Battlefield);g.put(1,1,Zone::Battlefield);
    cast(&mut g,2,vec![vec![Target::Player(1)]],"");
    cite!(["111.2","701.7b"];assert_eq!(tokens(&g).len(),2));
    for token in tokens(&g) {assert_eq!((g.obj(token).owner,g.controller(token)),(1,1));}
}

#[test]
fn r614_6_native_created_group_followup_reaches_every_actual_fractal() {
    for name in ["Bard, King of Dale","Doubling Season"] {
        let mut defs=native(&[name,"Additive Evolution"]);
        // Additive is full native text; Bard uses its exact clause while its draw replacement remains refused.
        for def in &defs {game::play::check(def).unwrap();}
        let mut g=Game::new(std::mem::take(&mut defs),2,20);g.players[0].pool.colors=[10;5];
        g.put(0,0,Zone::Battlefield);cast(&mut g,1,vec![],"WWWGG");resolve(&mut g);
        cite!(["614.6","608.2c"];assert_eq!(tokens(&g).len(),2));
        for token in tokens(&g) {assert_eq!(g.obj(token).counters.get("+1/+1"),Some(&if name=="Doubling Season" {6}else{3}));}
    }
}

#[test]
fn r701_63_full_season_distinguishes_endure_counter_cost_and_spirit_creation() {
    for payment in [true,false] {
        let mut defs=native(&["Doubling Season"]);
        defs.push(CardDef::new("Endurer","{0}","Creature","{0}: This creature endures 2.",Some((2,2))).unwrap());
        let mut g=Game::new(defs,2,20);g.put(0,0,Zone::Battlefield);let source=g.put(0,1,Zone::Battlefield);
        g.act(0,Move::Activate {source,ability:0,targets:vec![],paying:vec![]}).unwrap();resolve(&mut g);
        g.act(0,Move::Choose(if payment {Answer::Alternative(0)}else{Answer::No})).unwrap();
        cite!(["701.63a","118.12a","614.16"];assert_eq!(g.obj(source).counters.get("+1/+1").copied().unwrap_or(0),if payment {4}else{0}));
        assert_eq!(tokens(&g).len(),if payment {0}else{2});
        for token in tokens(&g) {assert_eq!(g.obj(token).chars.pt,Some((2,2)));}
    }
}

#[test]
fn r107_1b_creation_multiplier_overflow_restores_resolution_before_reserving_tokens() {
    let mut defs=native(&["Doubling Season"]);defs.push(spell("Create 2147483648 Treasure tokens."));
    let mut g=Game::new(defs,2,20);g.put(0,0,Zone::Battlefield);
    let card=g.put(0,1,Zone::Hand);g.act(0,Move::Cast {card,targets:vec![]}).unwrap();g.act(0,Move::Pass).unwrap();
    let before=format!("{g:?}");
    cite!(["107.1b","701.7b"];assert!(g.act(1,Move::Pass).unwrap_err().contains("107.1b")));
    assert_eq!(format!("{g:?}"),before);assert!(tokens(&g).is_empty());
}

#[test]
fn r701_7_zero_and_separate_instructions_do_not_invent_or_merge_creation_events() {
    let mut defs=native(&["Doubling Season"]);
    defs.push(spell("Create zero Treasure tokens. You gain 1 life."));
    defs.push(spell("Create a Treasure token. Create a Treasure token."));
    let mut g=Game::new(defs,2,20);g.put(0,0,Zone::Battlefield);
    cast(&mut g,1,vec![],"");
    cite!(["614.5","701.7a"];assert!(tokens(&g).is_empty() && g.decision.is_none()));
    assert_eq!(g.players[0].life,21);
    cast(&mut g,2,vec![],"");
    assert_eq!(tokens(&g).len(),4);
}

#[test]
fn r111_13_copied_permanent_spell_bypasses_creation_but_keeps_entry_replacement() {
    let mut defs=native(&["Doubling Season"]);
    defs.push(spell("Exile the top card of your library. Choose a card exiled this way and copy it. You may cast the copy without paying its mana cost."));
    defs.push(CardDef::new("Model","{0}","Creature","This creature enters with two +1/+1 counters on it.",Some((1,1))).unwrap());
    let mut g=Game::new(defs,2,20);g.put(0,0,Zone::Battlefield);g.put(0,2,Zone::Library);
    cast(&mut g,1,vec![],"");
    let Some(Decision::Choose {choice:Choice::Cast {cards},..})=g.decision.clone() else {panic!("copy offer")};
    g.act(0,Move::Cast {card:cards[0],targets:vec![]}).unwrap();resolve(&mut g);
    cite!(["111.13","608.3f"];assert_eq!(tokens(&g).len(),1));
    assert_eq!(g.obj(tokens(&g)[0]).counters.get("+1/+1"),Some(&4));
    assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Created {..})).count(),0);
}

#[test]
fn r614_6_created_groups_are_not_single_holder_endure_or_copy_sources() {
    for text in ["Create a 1/1 white Soldier creature token. It endures 2.",
                 "Create a 1/1 white Soldier creature token. Create a token that's a copy of it.",
                 "Create a 1/1 white Soldier creature token. You gain life equal to its power.",
                 "Create a 1/1 white Soldier creature token. If it's a creature, you gain 1 life.",
                 "Create a 1/1 white Soldier creature token. If it isn't a creature, it becomes a 2/2 Robot creature in addition to its other types. Otherwise, you gain 1 life."] {
        let def=spell(text);
        let reason=if text.ends_with("It endures 2.") {"endure: a singular permanent producer is required"}
            else if text.contains("copy of it") {"token copy: a multiplied created group is not a singular copy source"}
            else {"singular scalar or condition of a replacement-expanded created group"};
        crate::support::refused(game::play::check(&def).map_err(|e|e.0),reason);
    }
}

#[test]
fn r508_4_multiplied_attacking_tokens_have_independent_defender_choices() {
    let mut defs=native(&["Doubling Season"]);
    defs.push(spell("Create a 1/1 white Soldier creature token that's tapped and attacking."));
    defs.push(CardDef::planeswalker("Walker","{0}","Planeswalker","+1: You gain 1 life.",5).unwrap());
    defs.push(CardDef::new("Bear","{0}","Creature","",Some((2,2))).unwrap());
    let mut g=Game::new(defs,2,20);g.put(0,0,Zone::Battlefield);let bear=g.put(0,3,Zone::Battlefield);let walker=g.put(1,2,Zone::Battlefield);g.put_counters(walker,"loyalty",5);
    for _ in 0..5 {g.put(0,1,Zone::Library);g.put(1,1,Zone::Library);}
    while g.decision!=Some(Decision::Attackers {player:0}) {g.act(g.priority,Move::Pass).unwrap();}
    g.act(0,Move::Attack(vec![(bear,Target::Player(1))])).unwrap();
    cast(&mut g,1,vec![],"");
    for (i,target) in [Target::Player(1),Target::Object(walker)].into_iter().enumerate() {
        let Some(Decision::Choose {player:0,choice:Choice::Defender {nth,of,..}})=g.decision else {panic!("each actual token defender: {:?}",g.decision)};
        cite!(["508.4","614.6"];assert_eq!((nth,of),(i+1,2)));
        assert!(tokens(&g).is_empty());
        g.act(0,Move::Choose(Answer::Defender(target))).unwrap();
    }
    assert_eq!(tokens(&g).len(),2);
    let defenders=tokens(&g).iter().map(|id|g.combat.attackers.iter().find(|(on,_)|on==id).unwrap().1).collect::<Vec<_>>();
    assert_eq!(defenders,vec![Target::Player(1),Target::Object(walker)]);
    assert!(tokens(&g).iter().all(|id|g.obj(*id).tapped));
    assert!(g.log.iter().all(|event| !matches!(event,Event::Attacked {attacker,..} if tokens(&g).contains(attacker))));
}

#[test]
fn r614_12_every_multiplied_copy_keeps_its_independent_entry_choice() {
    let mut defs=native(&["Doubling Season"]);
    defs.push(spell("Create a token that's a copy of target creature."));
    defs.push(CardDef::new("Chooser","{0}","Creature","As this creature enters, choose a color.",Some((2,2))).unwrap());
    let mut g=Game::new(defs,2,20);g.put(0,0,Zone::Battlefield);let model=g.put(0,2,Zone::Battlefield);
    cast(&mut g,1,vec![vec![Target::Object(model)]],"");
    for color in [ir::Color::Red,ir::Color::Blue] {
        cite!(["701.7b","614.12"];assert!(matches!(g.decision,Some(Decision::Choose {player:0,choice:Choice::Value(game::binding::Kind::Color)}))));
        assert!(tokens(&g).is_empty());
        g.act(0,Move::Choose(Answer::Value(game::binding::Value::Color(color)))).unwrap();
    }
    assert_eq!(tokens(&g).len(),2);
    let choices=tokens(&g).iter().map(|id|g.obj(*id).choices.iter().map(|c|c.value.clone()).collect::<Vec<_>>()).collect::<Vec<_>>();
    assert!(choices[0].contains(&game::binding::Value::Color(ir::Color::Red)));
    assert!(choices[1].contains(&game::binding::Value::Color(ir::Color::Blue)));
    assert!(g.decision.is_none());
}

#[test]
fn r614_6_delayed_followup_retains_complete_actual_created_group() {
    let mut defs=native(&["Doubling Season"]);
    defs.push(spell("Create a 1/1 white Soldier creature token. Sacrifice it at the beginning of the next end step."));
    let mut g=Game::new(defs,2,20);g.put(0,0,Zone::Battlefield);cast(&mut g,1,vec![],"");
    assert_eq!(tokens(&g).len(),2);
    while g.part!=ir::Part::End {g.act(g.priority,Move::Pass).unwrap();}
    resolve(&mut g);
    cite!(["614.6","603.7"];assert!(tokens(&g).is_empty()));
    assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Created {..})).count(),2);
}

#[test]
fn r614_6_created_member_type_condition_changes_only_its_frozen_subset() {
    for selections in [[0,0],[1,1],[0,1]] {
        let mut defs=native(&["Doubling Season"]);
        defs.push(spell("Create a token that's a copy of target creature. If the token isn't a creature, it becomes a 2/2 Robot Villain creature in addition to its other types. Tap it."));
        defs.push(CardDef::new("Model","{0}","Creature","You may have this creature enter as a copy of any artifact on the battlefield.",Some((1,1))).unwrap());
        defs.push(CardDef::new("Rock","{0}","Artifact","",None).unwrap());
        defs.push(CardDef::new("Metal Bear","{0}","Artifact Creature — Bear","",Some((3,3))).unwrap());
        let mut g=Game::new(defs,2,20);g.put(0,0,Zone::Battlefield);
        let model=g.put(0,2,Zone::Battlefield);let rock=g.put(0,3,Zone::Battlefield);let bear=g.put(0,4,Zone::Battlefield);
        cast(&mut g,1,vec![vec![Target::Object(model)]],"");
        for selection in selections {
            assert!(matches!(g.decision,Some(Decision::Choose {choice:Choice::Select {..},..})));
            g.act(0,Move::Choose(Answer::Objects(vec![[rock,bear][selection]]))).unwrap();
        }
        let made=tokens(&g);assert_eq!(made.len(),2);
        for (token,selection) in made.into_iter().zip(selections) {
            cite!(["614.6","608.2c","707.2"];assert!(g.obj(token).tapped,"later it names the entire created group"));
            assert!(g.is_a(token,ir::CardType::Creature));
            assert_eq!((g.power(token),g.toughness(token)),if selection==0 {(Some(2),Some(2))} else {(Some(3),Some(3))});
            assert_eq!(g.seen(token,None).subtypes.iter().any(|subtype|subtype.name()=="Robot"),selection==0);
        }
    }
}
