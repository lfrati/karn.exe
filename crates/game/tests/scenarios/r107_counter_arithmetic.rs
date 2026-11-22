//! CR107.1b supplies no fixed numerical counter limit. Unrepresentable engine values
//! refuse explicitly; the public action preserves its exact prior proposal (733.1).
use crate::support::cite;
use game::{CardDef,Event,Game,Move,ObjId,Target,Zone};
use game::state::{Answer,Choice,Decision};
fn def(name:&str,kind:&str,text:&str)->CardDef {
    let d=CardDef::new(name,"{0}",kind,text,None).unwrap();
    game::play::check(&d).unwrap();d
}
fn game(amount:u32)->Game {
    Game::new(vec![
        def("Holder","Artifact",""),
        def("Place","Instant",&if amount==1 {"Put a charge counter on target artifact.".into()} else {format!("Put {amount} charge counters on target artifact.")}),
        def("Plus","Enchantment","If you would put one or more counters on a permanent, put that many plus one of each of those kinds of counters on it instead."),
        def("Double","Enchantment","If you would put one or more counters on a permanent, put twice that many of each of those kinds of counters on it instead."),
    ],2,20)
}
fn cast(g:&mut Game,target:ObjId) {
    let spell=g.put(0,1,Zone::Hand);
    g.act(0,Move::Cast{card:spell,targets:vec![vec![Target::Object(target)]]}).unwrap();
    g.act(0,Move::Pass).unwrap();
}
#[test]
fn r107_1b_holder_total_overflow_rejects_resolution_without_partial_effects() {
    let mut g=game(1);let holder=g.put(0,0,Zone::Battlefield);
    g.set_initial_counters(holder,"charge",u32::MAX);cast(&mut g,holder);
    let before=format!("{g:?}");
    cite!(["107.1b"];assert!(g.act(1,Move::Pass).unwrap_err().contains("engine numerical range")));
    assert_eq!(format!("{g:?}"),before);
}
#[test]
fn r107_1b_overflowing_replacement_answer_retains_exact_question_and_other_order() {
    let mut g=game(2_147_483_647);let holder=g.put(0,0,Zone::Battlefield);
    let plus=g.put(0,2,Zone::Battlefield);let double=g.put(0,3,Zone::Battlefield);
    cast(&mut g,holder);g.act(1,Move::Pass).unwrap();
    let Some(Decision::Choose{choice:Choice::Replacement{among,..},..})=&g.decision else {panic!("replacement choice")};
    let first=among.iter().find(|r|r.source==plus).unwrap().clone();
    let safe=among.iter().find(|r|r.source==double).unwrap().clone();
    let before=format!("{g:?}");
    cite!(["107.1b","616.1"];assert!(g.act(0,Move::Choose(Answer::Replacement(first))).unwrap_err().contains("engine numerical range")));
    assert_eq!(format!("{g:?}"),before);
    g.act(0,Move::Choose(Answer::Replacement(safe))).unwrap();
    assert_eq!(g.obj(holder).counters["charge"],u32::MAX);
}
#[test]
fn r107_1b_checked_fixture_accepts_boundary_and_rejects_next_without_events() {
    let mut g=game(1);let holder=g.put(0,0,Zone::Battlefield);
    g.try_put_counters(holder,"charge",u32::MAX).unwrap();
    let before=format!("{g:?}");
    // Uncited (contract): the finite representation refuses instead of clamping CR107.1b.
    crate::support::refused(g.try_put_counters(holder,"charge",1).map_err(|e| e.0), "counter amount exceeds the engine numerical range");
    assert_eq!(g.obj(holder).counters.get("charge"), Some(&u32::MAX));
    assert_eq!(format!("{g:?}"),before);
}
#[test]
fn r610_3_checked_return_completes_before_following_instruction() {
    let mut g=Game::new(vec![def("Prison","Enchantment","{0}: Exile target permanent until this enchantment leaves the battlefield."),def("Holder","Artifact",""),def("Release","Instant","Destroy target enchantment. Destroy all artifacts.")],2,20);
    let source=g.put(0,0,Zone::Battlefield);let holder=g.put(1,1,Zone::Battlefield);
    g.act(0,Move::Activate{source,ability:0,targets:vec![vec![holder.into()]],paying:vec![]}).unwrap();
    for _ in 0..2{g.act(g.priority,Move::Pass).unwrap();}
    let release=g.put(0,2,Zone::Hand);g.act(0,Move::Cast{card:release,targets:vec![vec![source.into()]]}).unwrap();
    for _ in 0..2{g.act(g.priority,Move::Pass).unwrap();}
    cite!(["610.3","608.2c"];assert!(g.battlefield.is_empty()));
    assert_eq!(g.players[1].graveyard.iter().filter(|o|g.obj(**o).card==Some(1)).count(),1);
}
#[test]
fn r610_3_return_entry_overflow_restores_parent_before_source_departure() {
    let mut g=Game::new(vec![def("Prison","Enchantment","{0}: Exile target permanent until this enchantment leaves the battlefield."),def("Holder","Artifact","This artifact enters with 2147483648 charge counters on it."),def("Release","Instant","Destroy target enchantment. You gain 1 life."),def("Double","Enchantment","If you would put one or more counters on a permanent, put twice that many of each of those kinds of counters on it instead.")],2,20);
    let source=g.put(0,0,Zone::Battlefield);let holder=g.put(1,1,Zone::Battlefield);
    g.act(0,Move::Activate{source,ability:0,targets:vec![vec![holder.into()]],paying:vec![]}).unwrap();
    for _ in 0..2{g.act(g.priority,Move::Pass).unwrap();}
    g.put(1,3,Zone::Battlefield);
    let release=g.put(0,2,Zone::Hand);g.act(0,Move::Cast{card:release,targets:vec![vec![source.into()]]}).unwrap();g.act(0,Move::Pass).unwrap();
    let before=format!("{g:?}");
    assert!(g.act(1,Move::Pass).unwrap_err().contains("engine numerical range"));
    assert_eq!(format!("{g:?}"),before);
    assert!(!g.log.iter().any(|e|matches!(e,Event::LifeGained{amount:1,..})));
}
#[test]
fn native_warden_derived_counter_total_cannot_be_preclamped_before_endure() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog=CardDef::catalog(&faces);
    let warden=CardDef::from_faces(&catalog["Warden of the Grove"]).unwrap();
    let bear=CardDef::new("Bear","{0}","Creature","",Some((2,2))).unwrap();
    let mut g=Game::new(vec![warden,bear],2,20);
    let w=g.put(0,0,Zone::Battlefield);let card=g.put(0,1,Zone::Hand);
    g.act(0,Move::Cast{card,targets:vec![]}).unwrap();
    for _ in 0..2{g.act(g.priority,Move::Pass).unwrap();}
    g.set_initial_counters(w,"charge",u32::MAX);g.set_initial_counters(w,"stun",1);
    g.act(0,Move::Pass).unwrap();let before=format!("{g:?}");
    cite!(["107.1b","701.63a"];assert!(g.act(1,Move::Pass).unwrap_err().contains("engine numerical range")));
    assert_eq!(format!("{g:?}"),before);
}
#[test]
fn r704_5_sba_source_death_return_overflow_restores_prior_public_action() {
    let mut g=Game::new(vec![
        CardDef::new("Prison","{0}","Creature","{0}: Exile target permanent until this creature leaves the battlefield.",Some((2,2))).unwrap(),
        def("Holder","Artifact","This artifact enters with 2147483648 charge counters on it."),
        def("Double","Enchantment","If you would put one or more counters on a permanent, put twice that many of each of those kinds of counters on it instead."),
    ],2,20);
    let source=g.put(0,0,Zone::Battlefield);let holder=g.put(1,1,Zone::Battlefield);
    g.act(0,Move::Activate{source,ability:0,targets:vec![vec![holder.into()]],paying:vec![]}).unwrap();
    for _ in 0..2{g.act(g.priority,Move::Pass).unwrap();}
    g.put(1,2,Zone::Battlefield);
    // A fixture marks lethal damage before the next public priority transition. This
    // isolates the SBA parent; the public pass must not leave a half-returned object.
    g.damage(source,source.into(),2);
    let before=format!("{g:?}");
    cite!(["704.5g","610.3","107.1b"];assert!(g.act(0,Move::Pass).unwrap_err().contains("engine numerical range")));
    assert_eq!(format!("{g:?}"),before);
}
#[test]
fn r118_11_pending_loyalty_replacement_overflow_restores_cost_and_safe_order_pays_once() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let catalog=CardDef::catalog(&faces);
    let doc=CardDef::from_faces(&catalog["Doc Samson, Super Psychiatrist"]).unwrap();
    let loading=CardDef::from_faces(&catalog["Loading Zone"]).unwrap();
    // A synthetic Spacecraft planeswalker matches Loading Zone's actual recipient domain.
    let walker=CardDef::planeswalker("Walker","{0}","Artifact Planeswalker — Jace Spacecraft","+1: You gain 1 life.",1).unwrap();
    let mut g=Game::new(vec![walker,doc,loading],2,20);
    let source=g.put(0,0,Zone::Battlefield);let plus=g.put(0,1,Zone::Battlefield);let double=g.put(0,2,Zone::Battlefield);
    g.set_initial_counters(source,"loyalty",u32::MAX-3);
    g.act(0,Move::Activate{source,ability:0,targets:vec![],paying:vec![]}).unwrap();
    let Some(Decision::Choose{choice:Choice::Replacement{among,..},..})=&g.decision else{panic!("counter cost ordering: {:?}",g.decision)};
    let first=among.iter().find(|r|r.source==plus).unwrap().clone();let safe=among.iter().find(|r|r.source==double).unwrap().clone();
    let before=format!("{g:?}");
    cite!(["606.4","118.11","107.1b"];assert!(g.act(0,Move::Choose(Answer::Replacement(first))).unwrap_err().contains("engine numerical range")));
    assert_eq!(format!("{g:?}"),before);
    g.act(0,Move::Choose(Answer::Replacement(safe))).unwrap();
    assert_eq!(g.obj(source).counters["loyalty"],u32::MAX);
    assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Activated{..})).count(),1);
    for _ in 0..2{g.act(g.priority,Move::Pass).unwrap();}
    assert_eq!(g.players[0].life,21);
}
