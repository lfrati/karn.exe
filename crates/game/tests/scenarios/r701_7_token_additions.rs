//! Append-one token creation and ordinary Case solving: pinned Quina/Helm rulings and CR 719.3.
use crate::support::cite;
use game::{CardDef,Game,Move,ObjId,Target,Zone};
use game::state::{Answer,Choice,Decision};


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
fn cast(g:&mut Game,card:usize,targets:Vec<Vec<Target>>) {
    let card=g.put(0,card,Zone::Hand);g.act(0,Move::Cast {card,targets}).unwrap();resolve(g);
}
fn choose(g:&mut Game,source:ObjId) {
    let Some(Decision::Choose {player,choice:Choice::Replacement {among,..}})=g.decision.clone() else {panic!("creation ordering: {:?}",g.decision)};
    let id=among.into_iter().find(|id|id.source==source).expect("authored source offered");
    g.act(player,Move::Choose(Answer::Replacement(id))).unwrap();
}
fn tokens(g:&Game)->Vec<ObjId> {g.battlefield.iter().copied().filter(|on|g.obj(*on).token()).collect()}

// Uncited (admission): the full pinned definitions compile; nothing is claimed of their play.
#[test]
fn native_consumers_compile() {
    for def in native(&["Quina, Qu Gourmet","Worldwalker Helm","Case of the Pilfered Proof"]) {
        game::play::check(&def).unwrap_or_else(|why|panic!("{}: {}",def.name(),why.0));
    }
}

// Rules: 614.5, 614.6, 701.7a.
#[test]
fn r614_5_append_once_for_a_mixed_inventory_and_separately_for_next_instruction() {
    let mut defs=native(&["Quina, Qu Gourmet"]);
    defs.push(spell("Create two Treasure tokens and three 1/1 white Soldier creature tokens. Create a Treasure token."));
    let mut g=Game::new(defs,2,20);g.put(0,0,Zone::Battlefield);cast(&mut g,1,vec![]);
    cite!(["614.5","614.6","701.7a"];assert_eq!(tokens(&g).len(),8));
    cite!(["614.5"];assert_eq!(tokens(&g).iter().filter(|on|g.obj(**on).chars.name=="Frog Token").count(),2));
}

// Rules: 614.5, 701.7a, 110.5a.
#[test]
fn r701_7_zero_and_another_creator_do_not_get_a_frog() {
    let mut defs=native(&["Quina, Qu Gourmet"]);
    defs.push(spell("Create zero Treasure tokens. Target opponent creates a Treasure token."));
    let mut g=Game::new(defs,2,20);g.put(0,0,Zone::Battlefield);cast(&mut g,1,vec![vec![Target::Player(1)]]);
    cite!(["614.5","701.7a","110.5a"];assert_eq!(tokens(&g).len(),1));
    cite!(["110.5a"];assert_eq!(g.controller(tokens(&g)[0]),1));
}

// Rules: 616.1, 616.1f, 614.5, 701.7b.
#[test]
fn r616_1_append_and_multiply_have_different_legal_orders() {
    for append_first in [true,false] {
        let mut defs=native(&["Quina, Qu Gourmet","Doubling Season"]);
        defs.push(spell("Create two Treasure tokens."));
        let mut g=Game::new(defs,2,20);let q=g.put(0,0,Zone::Battlefield);let d=g.put(0,1,Zone::Battlefield);
        cast(&mut g,2,vec![]);choose(&mut g,if append_first {q}else{d});
        cite!(["616.1","616.1f","614.5"];assert_eq!(tokens(&g).len(),if append_first {6}else{5}));
        cite!(["701.7b"];assert_eq!(tokens(&g).iter().filter(|on|g.obj(**on).chars.name=="Frog Token").count(),if append_first {2}else{1}));
    }
}

// Rules: 616.1f, 701.7b.
#[test]
fn r616_1f_append_frog_makes_creature_multiplier_newly_applicable() {
    let mut defs=native(&["Quina, Qu Gourmet","Ojer Taq, Deepest Foundation"]);
    defs.push(spell("Create two Treasure tokens."));
    let mut g=Game::new(defs,2,20);g.put(0,0,Zone::Battlefield);g.put(0,1,Zone::Battlefield);cast(&mut g,2,vec![]);
    cite!(["616.1f","701.7b"];assert_eq!(tokens(&g).len(),5));
    cite!(["701.7b"];assert_eq!(tokens(&g).iter().filter(|on|g.obj(**on).chars.name=="Frog Token").count(),3));
}

// Rules: 701.7b, 614.5.
#[test]
fn r701_7b_helm_qualifies_from_base_artifact_not_creature_inventory() {
    for (text,expected) in [("Create two Treasure tokens.",3),("Create two 1/1 white Soldier creature tokens.",2)] {
        let mut defs=native(&["Worldwalker Helm"]);defs.push(spell(text));
        let mut g=Game::new(defs,2,20);g.put(0,0,Zone::Battlefield);cast(&mut g,1,vec![]);
        cite!(["701.7b","614.5"];assert_eq!(tokens(&g).len(),expected));
    }
}

// Rules: 111.3, 614.6, 603.7, 701.7a.
#[test]
fn r614_6_additional_frog_gets_entry_instructions_and_cleanup_but_not_original_abilities() {
    let mut defs=native(&["Quina, Qu Gourmet"]);
    defs.push(spell("Create a tapped 1/1 white Soldier creature token with flying. Exile it at the beginning of the next end step."));
    let mut g=Game::new(defs,2,20);g.put(0,0,Zone::Battlefield);cast(&mut g,1,vec![]);
    let made=tokens(&g);
    cite!(["614.6","701.7a"];assert_eq!(made.len(),2));
    for on in &made {
        cite!(["614.6"];assert!(g.obj(*on).tapped));
        cite!(["111.3"];assert_eq!(g.has(*on,"Flying"),g.obj(*on).chars.name=="Soldier Token"));
    }
    end_step(&mut g);
    resolve(&mut g);
    cite!(["614.6","603.7"];assert!(tokens(&g).is_empty()));
}

fn case_board() -> (Game,ObjId,Vec<ObjId>) {
    let mut defs=native(&["Case of the Pilfered Proof"]);
    defs.push(CardDef::new("Detective","{0}","Creature — Detective","",Some((1,1))).unwrap());
    defs.push(spell("Create a Treasure token."));
    defs.push(spell("Destroy target creature."));
    defs.push(spell("Create a token that's a copy of target enchantment."));
    defs.push(spell("Gain control of target permanent."));
    defs.push(spell("Target enchantment becomes a copy of target enchantment."));
    let mut g=Game::new(defs,2,20);let case=g.put(0,0,Zone::Battlefield);
    let detectives=(0..3).map(|_|g.put(0,1,Zone::Battlefield)).collect();(g,case,detectives)
}
fn end_step(g:&mut Game) {while g.part!=ir::Part::End {if matches!(g.decision,Some(Decision::Attackers {..})) {g.act(g.active,Move::Attack(vec![])).unwrap();}else{g.act(g.priority,Move::Pass).unwrap();}}}

// Rules: 719.3a, 719.3b, 702.169b, 603.4, 614.5.
#[test]
fn r719_3_case_solves_ordinarily_and_enables_clue_addition() {
    let (mut g,case,_)=case_board();
    cast(&mut g,2,vec![]);
    cite!(["702.169b"];assert_eq!(tokens(&g).len(),1));
    end_step(&mut g);
    cite!(["719.3a","603.4"];assert_eq!(g.stack.len(),1));
    cite!(["719.3a"];assert!(!g.obj(case).solved));
    resolve(&mut g);
    cite!(["719.3b"];assert!(g.obj(case).solved));
    cast(&mut g,2,vec![]);
    cite!(["702.169b","614.5"];assert_eq!(tokens(&g).len(),3));
}

// Rules: 719.3a, 603.4, 608.2a.
#[test]
fn r719_3_case_intervening_if_rechecks_detectives_on_resolution() {
    let (mut g,case,detectives)=case_board();end_step(&mut g);
    let card=g.put(0,3,Zone::Hand);g.act(0,Move::Cast {card,targets:vec![vec![detectives[0].into()]]}).unwrap();resolve(&mut g);resolve(&mut g);
    cite!(["719.3a","603.4","608.2a"];assert!(!g.obj(case).solved));
    cast(&mut g,2,vec![]);
    cite!(["702.169b"];assert_eq!(tokens(&g).len(),1));
}

// Rules: 107.1b, 701.7b, 614.6.
#[test]
fn r107_1b_append_overflow_and_heterogeneous_instruction_refusal_restore_resolution() {
    for (text,reason) in [("Create 4294967295 Treasure tokens.","engine numerical range"),
        ("Create a tapped Treasure token and a 1/1 white Soldier creature token.","heterogeneous token entry instructions")] {
        let mut defs=native(&["Quina, Qu Gourmet"]);defs.push(spell(text));
        let mut g=Game::new(defs,2,20);g.put(0,0,Zone::Battlefield);
        let card=g.put(0,1,Zone::Hand);g.act(0,Move::Cast {card,targets:vec![]}).unwrap();g.act(0,Move::Pass).unwrap();
        let before=format!("{g:?}");
        // Uncited (support contract): refuse this named unimplemented heterogeneous boundary.
        assert!(g.act(1,Move::Pass).unwrap_err().contains(reason));
        cite!(["107.1b","701.7b","614.6"];assert_eq!(format!("{g:?}"),before));
        cite!(["701.7b"];assert!(tokens(&g).is_empty()));
    }
    // A mixed instruction inventory without an append effect still preserves both originals.
    let mut g=Game::new(vec![spell("Create a tapped Treasure token and a 1/1 white Soldier creature token.")],2,20);
    cast(&mut g,0,vec![]);
    cite!(["701.7a"];assert_eq!(tokens(&g).iter().map(|on|g.obj(*on).tapped).collect::<Vec<_>>(),vec![true,false]));
}

// Rules: 719.3a, 603.4.
#[test]
fn r719_3_false_condition_at_end_step_does_not_trigger() {
    let (mut g,case,detectives)=case_board();
    cast(&mut g,3,vec![vec![detectives[0].into()]]);end_step(&mut g);
    cite!(["719.3a","603.4"];assert!(g.stack.is_empty() && !g.obj(case).solved));
}

// Rules: 719.3b, 707.2, 400.7, 702.169b.
#[test]
fn r719_3_solved_is_not_copied_and_resets_on_new_incarnation() {
    let (mut g,case,_)=case_board();end_step(&mut g);resolve(&mut g);
    cite!(["719.3b"];assert!(g.obj(case).solved));
    cast(&mut g,4,vec![vec![case.into()]]);
    let copy=tokens(&g).into_iter().find(|on|g.obj(*on).chars.name=="Case of the Pilfered Proof").unwrap();
    cite!(["719.3b","707.2"];assert!(!g.obj(copy).solved));
    cast(&mut g,6,vec![vec![case.into()],vec![copy.into()]]);
    cite!(["719.3b","707.2"];assert!(g.obj(case).solved));
    // Fixture movement is a zone transition, not a cast/ETB witness.
    g.move_object(case,Zone::Hand);
    let now=g.players[0].hand.iter().copied().find(|on|g.obj(*on).chars.name=="Case of the Pilfered Proof").unwrap();
    cite!(["719.3b","400.7"];assert!(!g.obj(now).solved));
    // Reentry is an actual cast; it cannot recover its departed incarnation's solved marker.
    // Uncited (fixture): move the decision to an own main phase; the assertions concern incarnation reset.
    g.part=ir::Part::PostcombatMain;
    g.players[0].pool.colors=[5;5];g.act(0,crate::support::paid("WW", Move::Cast {card:now,targets:vec![]})).unwrap();resolve(&mut g);
    let case=g.battlefield.iter().copied().find(|on|g.obj(*on).chars.name=="Case of the Pilfered Proof").unwrap();
    cite!(["719.3b","400.7"];assert!(!g.obj(case).solved));
}

// Rules: 719.3b, 110.2a, 702.169b.
#[test]
fn r719_3_solved_survives_control_change_and_benefits_new_controller() {
    let (mut g,case,_)=case_board();end_step(&mut g);resolve(&mut g);
    g.act(0,Move::Pass).unwrap();
    let card=g.put(1,5,Zone::Hand);g.act(1,Move::Cast {card,targets:vec![vec![case.into()]]}).unwrap();resolve(&mut g);
    cite!(["719.3b","110.2a"];assert!(g.obj(case).solved && g.controller(case)==1));
    g.act(0,Move::Pass).unwrap();
    let card=g.put(1,2,Zone::Hand);g.act(1,Move::Cast {card,targets:vec![]}).unwrap();resolve(&mut g);
    cite!(["702.169b"];assert_eq!(tokens(&g).len(),2));
    cite!(["110.2a"];assert!(tokens(&g).iter().all(|on|g.controller(*on)==1)));
}

// Rules: 701.7b, 614.12, 707.2.
#[test]
fn r701_7b_helm_uses_creation_values_before_an_entry_copy_choice() {
    let mut defs=native(&["Worldwalker Helm"]);
    defs.push(spell("Create a token that's a copy of target creature."));
    defs.push(CardDef::new("Model","{0}","Creature","You may have this creature enter as a copy of any artifact on the battlefield.",Some((2,2))).unwrap());
    defs.push(CardDef::new("Rock","{0}","Artifact","",None).unwrap());
    let mut g=Game::new(defs,2,20);g.put(0,0,Zone::Battlefield);let model=g.put(0,2,Zone::Battlefield);let rock=g.put(0,3,Zone::Battlefield);
    cast(&mut g,1,vec![vec![model.into()]]);
    cite!(["614.12","701.7b"];assert!(matches!(g.decision,Some(Decision::Choose {player:0,choice:Choice::Select {..}}))));
    cite!(["701.7b"];assert!(tokens(&g).is_empty()));
    g.act(0,Move::Choose(Answer::Objects(vec![rock]))).unwrap();
    cite!(["701.7b","707.2"];assert_eq!(tokens(&g).len(),1));
    cite!(["707.2"];assert_eq!(g.obj(tokens(&g)[0]).chars.name,"Rock"));
}

// Rules: 508.4, 701.7b, 616.1, 111.10s.
#[test]
fn r508_4_added_frog_gets_its_own_defender_but_added_map_cannot_attack() {
    let mut defs=native(&["Quina, Qu Gourmet","Worldwalker Helm"]);
    defs.push(spell("Create a 1/1 colorless Thopter artifact creature token that's tapped and attacking."));
    defs.push(CardDef::planeswalker("Walker","{0}","Planeswalker","+1: You gain 1 life.",5).unwrap());
    let mut g=Game::new(defs,2,20);let walker=g.put(1,3,Zone::Battlefield);g.put_counters(walker,"loyalty",5);let q=g.put(0,0,Zone::Battlefield);g.put(0,1,Zone::Battlefield);
    g.part=ir::Part::DeclareAttackers;
    cast(&mut g,2,vec![]);choose(&mut g,q);
    for target in [Target::Player(1),Target::Object(walker)] {
        cite!(["508.4","701.7b"];assert!(matches!(&g.decision,Some(Decision::Choose {player:0,choice:Choice::Defender {among,..}}) if among.len()==2 && among.contains(&Target::Player(1)) && among.contains(&Target::Object(walker)))));
        cite!(["701.7b"];assert!(tokens(&g).is_empty()));
        g.act(0,Move::Choose(Answer::Defender(target))).unwrap();
    }
    cite!(["508.4","111.10s"];assert_eq!((tokens(&g).len(),g.combat.attackers.len()),(3,2)));
    cite!(["701.7a"];assert!(tokens(&g).iter().all(|on|g.obj(*on).tapped)));
}

// Rules: 303.4f, 701.3b, 701.7a.
#[test]
fn r701_3b_added_frog_cannot_attach_like_the_original_role() {
    let mut defs=native(&["Quina, Qu Gourmet"]);
    defs.push(spell("Create a Monster Role token attached to target creature you control."));
    let mut g=Game::new(defs,2,20);let q=g.put(0,0,Zone::Battlefield);cast(&mut g,1,vec![vec![q.into()]]);
    cite!(["701.7a"];assert_eq!(tokens(&g).len(),2));
    for on in tokens(&g) {
        cite!(["303.4f","701.3b"];assert_eq!(g.attached_to(on),if g.obj(on).chars.name=="Monster" {Some(q.into())}else{None}));
    }
}

// Rules: 111.13, 608.3f, 614.5.
#[test]
fn r111_13_permanent_spell_copy_is_a_token_without_append_creation() {
    let mut defs=native(&["Quina, Qu Gourmet"]);
    defs.push(spell("Exile the top card of your library. Choose a card exiled this way and copy it. You may cast the copy without paying its mana cost."));
    defs.push(CardDef::new("Model","{0}","Creature","",Some((1,1))).unwrap());
    let mut g=Game::new(defs,2,20);g.put(0,0,Zone::Battlefield);g.put(0,2,Zone::Library);cast(&mut g,1,vec![]);
    let Some(Decision::Choose {choice:Choice::Cast {cards},..})=g.decision.clone() else {panic!("copy offer")};
    g.act(0,Move::Cast {card:cards[0],targets:vec![]}).unwrap();resolve(&mut g);
    cite!(["111.13","608.3f","614.5"];assert_eq!(tokens(&g).len(),1));
    cite!(["111.13"];assert!(g.log.iter().all(|e|!matches!(e,game::Event::Created {..}))));
}

// Rules: 111.10s, 707.2, 701.44a, 614.5.
#[test]
fn r111_10s_helm_copies_an_artifact_token_and_maps_have_their_own_ability() {
    let mut defs=native(&["Worldwalker Helm"]);
    defs.push(spell("Create a Treasure token."));
    defs.push(CardDef::new("Bear","{0}","Creature — Bear","",Some((2,2))).unwrap());
    defs.push(CardDef::new("Forest","","Basic Land — Forest","",None).unwrap());
    let mut g=Game::new(defs,2,20);let helm=g.put(0,0,Zone::Battlefield);let bear=g.put(0,2,Zone::Battlefield);
    g.put(0,3,Zone::Library);cast(&mut g,1,vec![]);
    let treasure=tokens(&g).into_iter().find(|on|g.obj(*on).chars.name=="Treasure Token").unwrap();
    g.players[0].pool.colors=[5;5];
    g.act(0,crate::support::paid("WU", Move::Activate {source:helm,ability:1,targets:vec![vec![treasure.into()]],paying:vec![]})).unwrap();resolve(&mut g);
    cite!(["707.2","614.5"];assert_eq!(tokens(&g).len(),4));
    let map=tokens(&g).into_iter().find(|on|g.obj(*on).chars.name=="Map Token").unwrap();
    g.act(0,crate::support::paid("W", Move::Activate {source:map,ability:0,targets:vec![vec![bear.into()]],paying:vec![]})).unwrap();resolve(&mut g);
    cite!(["111.10s","701.44a"];assert_eq!(g.players[0].hand.len(),1));
    cite!(["111.10s"];assert!(!g.battlefield.contains(&map)));
}

// Rules: 109.5, 603.3a, 603.4, 719.3a.
#[test]
fn r719_3_solve_recheck_uses_trigger_controller_after_source_changes_control() {
    let (mut g,case,_)=case_board();end_step(&mut g);
    cite!(["719.3a","603.3a"];assert_eq!(g.stack.len(),1));
    g.act(0,Move::Pass).unwrap();
    let steal=g.put(1,5,Zone::Hand);
    g.act(1,Move::Cast {card:steal,targets:vec![vec![case.into()]]}).unwrap();resolve(&mut g);
    cite!(["603.3a"];assert!(g.controller(case)==1 && !g.obj(case).solved));
    // B controls no Detectives. The pending trigger still belongs to A, whose three
    // Detectives satisfy its captured "you" at resolution (109.5), so it solves.
    resolve(&mut g);
    cite!(["109.5","603.3a","603.4","719.3a"];assert!(g.obj(case).solved));
}

// Uncited (initialization contract): no past event or chosen entry property is inferred.
#[test]
fn settled_creation_replacements_need_no_unrepresented_initial_history() {
    let defs=native(&["Quina, Qu Gourmet","Worldwalker Helm","Doubling Season","Case of the Pilfered Proof"]);
    let mut g=Game::new(defs,2,20);
    let controls=(0..4).map(|index|(g.put(0,index,Zone::Battlefield),0,false)).collect::<Vec<_>>();
    g.settle_initial_position(3,ir::Part::PostcombatMain,0,0,0,&controls).unwrap();
    assert!(!g.obj(controls[3].0).solved);
    assert!(g.stack.is_empty());
}
