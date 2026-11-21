//! Entwine changes the announced mode count and pays an additional cost (702.42, 601.2).
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};
use std::sync::OnceLock;
fn full(name: &str) -> CardDef {
    static FACES: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    let faces=FACES.get_or_init(||oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    let d=CardDef::from_faces(&CardDef::catalog(faces)[name]).unwrap();
    game::play::check(&d).unwrap_or_else(|e|panic!("{name}: {}",e.0)); d
}
fn setup(names: &[&str]) -> Game { Game::new(names.iter().map(|n|full(n)).collect::<Vec<_>>(),2,20) }
fn pass(g: &mut Game) { for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();} }
fn modal(card: ObjId, paid: bool, modes: &[usize], targets: Vec<Vec<Target>>) -> Move {
    Move::WithEntwine(paid, Box::new(Move::Modes(modes.to_vec(),Box::new(Move::Cast{card,targets}))))
}
fn mana(g: &mut Game, n: u32) { g.players[0].pool.colors[ir::Color::Green as usize]=n; }
fn choose(g: &mut Game, chosen: Vec<ObjId>) { g.act(0,Move::Choose(Answer::Objects(chosen))).unwrap(); }
#[test]
fn full_cards_and_fixed_spell_only_boundary() {
    setup(&["Tooth and Nail","Rude Awakening","Goblin War Party","Barbed Lightning"]);
    for name in ["Solar Tide","Betrayal of Flesh"] {
        let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        let d=CardDef::from_faces(&CardDef::catalog(&faces)[name]).unwrap();
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),"keyword: Entwine");
    }
    for text in ["Choose one —\n• You gain 1 life.\n• You gain 2 life.\nEntwine {X}",
        "You gain 1 life.\nEntwine {1}","Choose one —\n• You gain 1 life.\n• You gain 2 life.\nEntwine {1}\nEntwine {2}"] {
        let d=CardDef::new("Unsupported","{0}","Sorcery",text,None).unwrap();let reason=if text.contains("{X}") { "keyword: Entwine" } else if text.matches("Entwine").count()==2 { "multiple entwine costs require separately identified announcements (702.42)" } else { "entwine requires a modal spell (702.42)" };
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
    }
}
#[test]
fn paid_and_unpaid_offers_costs_and_atomic_invalid_modes() {
    let mut g=setup(&["Tooth and Nail"]);let card=g.put(0,0,Zone::Hand);mana(&mut g,7);
    for modes in [vec![0], vec![1]] {
        cite!(["702.42a"]; ok: g.clone().act(0, crate::support::paid("GGGGGGG", modal(card, false, &modes, vec![]))));
    }
    // entwine unaffordable; both modes unentwined; entwined with one, a repeat, a mode it lacks
    for (paid,modes,why) in [(true,vec![0,1],"your mana pool cannot pay that cost (118.3a): {7}{G}{G}"),(false,vec![0,1],"choose 1 to 1 of its modes (700.2a)"),(true,vec![0],"choose 2 to 2 of its modes (700.2a)"),(true,vec![0,0],"a mode is chosen once (700.2d)"),(true,vec![0,2],"a mode the ability does not have")] {
        let before=format!("{g:?}");cite!(["702.42a"]; case: why; refused(why): g.act(0,modal(card,paid,&modes,vec![])));assert_eq!(format!("{g:?}"),before);
    }
    mana(&mut g,9);
    let before=format!("{g:?}");cite!(["601.2b"]; refused("announce entwine only once (601.2b)"): g.act(0,Move::WithEntwine(true,Box::new(modal(card,false,&[0],vec![])))));assert_eq!(format!("{g:?}"),before);
    g.act(0,crate::support::paid("GGGGGGGGG",modal(card,true,&[1,0],vec![]))).unwrap();let spell=g.stack[0];assert!(g.obj(spell).entwined);
    cite!(["702.42a"]; assert_eq!(g.obj(spell).modes,vec![0,1]));cite!(["702.42a"]; assert_eq!(g.players[0].pool.total(),0));
}
#[test]
fn full_tooth_search_then_independent_hand_choice_survives_suspension_and_clone() {
    let mut g=setup(&["Tooth and Nail","Grizzly Bears","Llanowar Elves","Forest"]);
    let card=g.put(0,0,Zone::Hand);g.put(0,1,Zone::Library);g.put(0,2,Zone::Library);g.put(0,3,Zone::Library);
    g.put(0,1,Zone::Hand);mana(&mut g,9);g.act(0,crate::support::paid("GGGGGGGGG",modal(card,true,&[1,0],vec![]))).unwrap();pass(&mut g);
    let Some(Decision::Choose{choice:Choice::Search{among,min,max,..},..})=&g.decision else{panic!("{:?}",g.decision)};
    cite!(["702.42b"]; assert_eq!((*min,*max),(0,2)));assert_eq!(among.len(),2);let found=among.clone();
    for mut g in [g.clone(),g] {
        choose(&mut g,found.clone());assert_eq!(g.players[0].library.len(),1);assert_eq!(g.players[0].hand.len(),3);
        let hand=g.players[0].hand.clone();let bear=*hand.iter().find(|o|g.obj(**o).card==Some(1)).unwrap();
        let elf=*hand.iter().find(|o|g.obj(**o).card==Some(2)).unwrap();choose(&mut g,vec![bear,elf]);
        cite!(["702.42b"]; assert_eq!(g.battlefield.len(),2));assert_eq!(g.players[0].hand.len(),1);assert!(g.stack.is_empty());
    }
}
#[test]
fn full_tooth_unpaid_single_modes_and_empty_choices() {
    for modes in [vec![0],vec![1]] {
        let mut g=setup(&["Tooth and Nail","Grizzly Bears"]);let card=g.put(0,0,Zone::Hand);
        g.put(0,1,Zone::Library);g.put(0,1,Zone::Hand);mana(&mut g,7);
        g.act(0,crate::support::paid("GGGGGGG",modal(card,false,&modes,vec![]))).unwrap();cite!(["702.42a"]; case: format!("{modes:?}"); assert_eq!(g.players[0].pool.total(),0));pass(&mut g);
        choose(&mut g,vec![]);assert!(g.stack.is_empty());assert!(g.battlefield.is_empty());assert_eq!(g.players[0].hand.len(),1);
    }
}
#[test]
fn full_rude_untaps_then_animates_still_lands_until_cleanup() {
    let mut g=setup(&["Rude Awakening","Forest"]);let land=g.put(0,1,Zone::Battlefield);g.set_tapped(land,true);
    for _ in 0..10{g.put(0,1,Zone::Library);g.put(1,1,Zone::Library);}
    let card=g.put(0,0,Zone::Hand);mana(&mut g,8);g.act(0,crate::support::paid("GGGGGGGG",modal(card,true,&[1,0],vec![]))).unwrap();cite!(["702.42a"]; assert_eq!(g.players[0].pool.total(),0));pass(&mut g);
    assert!(!g.obj(land).tapped);cite!(["702.42b"]; assert_eq!((g.power(land),g.toughness(land)),(Some(2),Some(2))));
    cite!(["305.6", "605.1a"]; ok: g.clone().act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }));
    let turn=g.turn;
    for _ in 0..100{if g.turn>turn{break;}match g.decision {Some(Decision::Attackers{player})=>g.act(player,Move::Attack(vec![])).unwrap(),Some(Decision::Blockers{player})=>g.act(player,Move::Block(vec![])).unwrap(),_=>g.act(g.priority,Move::Pass).unwrap()}}
    assert!(g.turn>turn);assert_eq!(g.power(land),None);
}
#[test]
fn full_goblin_war_party_applies_second_mode_to_new_tokens() {
    let mut g=setup(&["Goblin War Party"]);let card=g.put(0,0,Zone::Hand);g.players[0].pool.colors[ir::Color::Red as usize]=7;
    g.act(0,crate::support::paid("RRRRRRR",modal(card,true,&[1,0],vec![]))).unwrap();pass(&mut g);assert_eq!(g.battlefield.len(),3);
    for o in &g.battlefield{cite!(["702.42b"]; assert_eq!((g.power(*o),g.toughness(*o)),(Some(2),Some(2))));assert!(g.has(*o,"Haste"));}
}
#[test]
fn full_barbed_targets_and_copies_keep_both_modes_without_second_payment() {
    let mut g=setup(&["Barbed Lightning","Craw Wurm","Twincast"]);let wurm=g.put(1,1,Zone::Battlefield);let card=g.put(0,0,Zone::Hand);
    g.players[0].pool.colors[ir::Color::Red as usize]=5;g.act(0,crate::support::paid("RRRRR",modal(card,true,&[1,0],vec![vec![Target::Object(wurm)],vec![Target::Player(1)]]))).unwrap();
    let original=g.stack[0];g.players[0].pool.colors[ir::Color::Blue as usize]=2;let twin=g.put(0,2,Zone::Hand);
    g.act(0,crate::support::paid("UU",Move::Cast{card:twin,targets:vec![vec![Target::Object(original)]]})).unwrap();pass(&mut g);
    assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::Retarget{..},..})));
    for mut g in [g.clone(),g] {
        g.act(0,Move::Choose(Answer::No)).unwrap();let copy=*g.stack.last().unwrap();assert_ne!(copy,original);
        cite!(["700.2g"]; assert_eq!(g.obj(copy).modes,vec![0,1]));cite!(["707.10"]; assert!(g.obj(copy).entwined));assert_eq!(g.players[0].pool.total(),0);
        pass(&mut g);assert_eq!((g.obj(wurm).damage,g.players[1].life),(3,17));pass(&mut g);
        assert_eq!(g.players[1].life,14);assert!(!g.battlefield.contains(&wurm));assert!(g.stack.is_empty());
    }
}

#[test]
fn r601_2g_paid_modes_survive_mana_child_clone_and_reversal() {
    use game::legal::Step;
    let texts=[("Mana creature","Creature — Elf","{T}: Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature."),
        ("Scales","Enchantment","If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead."),
        ("Evolution","Enchantment","If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.")];
    let mut defs=Vec::new();for(n,t,x) in texts{let d=CardDef::new(n,"{0}",t,x,if t.starts_with("Creature"){Some((1,1))}else{None}).unwrap();game::play::check(&d).unwrap();defs.push(d);}
    defs.push(CardDef::new("Paid modes","{1}","Sorcery","Choose one —\n• You gain 1 life.\n• You gain 2 life.\nEntwine {3}",None).unwrap());
    let mut g=Game::new(defs,2,20);let source=g.put(0,0,Zone::Battlefield);g.put(0,1,Zone::Battlefield);let evolution=g.put(0,2,Zone::Battlefield);let card=g.put(0,3,Zone::Hand);
    let step=Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source,ability:0,paying:vec![],adding:vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Blue);4]]};
    g.act(0,Move::WithMana(vec![step],Box::new(modal(card,true,&[1,0],vec![])))).unwrap();let spell=g.stack[0];
    assert!(g.obj(spell).entwined);cite!(["702.42a"]; assert_eq!(g.obj(spell).modes,vec![0,1]));
    let Some(Decision::Choose{choice:Choice::Replacement{among,..},..})=&g.decision else{panic!("{:?}",g.decision)};
    let effect=among.iter().find(|e|e.source==evolution).unwrap().clone();
    for mut g in [g.clone(),g]{
        g.act(0,Move::Choose(Answer::Replacement(effect.clone()))).unwrap();
        if matches!(g.decision,Some(Decision::Choose{choice:Choice::Mana{..},..})){g.act(0,Move::Choose(Answer::Mana(vec![ir::mana::ManaSymbol::Color(ir::Color::Blue);3]))).unwrap();}
        assert!(matches!(g.decision,Some(Decision::Payment{..})));let mut corrected=g.clone();corrected.players[0].pool.colorless+=1;
        corrected.act(0,Move::FinishPayment{plan:vec![],paying:vec![],with:Some(crate::support::mana_pool("UUUC"))}).unwrap();pass(&mut corrected);cite!(["702.42b"]; assert_eq!(corrected.players[0].life,23));
        g.act(0,Move::ReversePayment).unwrap();assert_eq!(g.obj(card).zone,Zone::Hand);assert!(!g.obj(card).entwined);assert!(g.stack.is_empty());assert!(g.obj(source).tapped);
    }
}
#[test]
fn r118_9_free_cast_still_pays_entwine_and_selects_every_mode() {
    let offer=CardDef::new("Offer","{0}","Instant","Exile the top card of your library. You may cast that card without paying its mana cost.",None).unwrap();
    let modes=CardDef::new("Expensive","{9}","Sorcery","Choose one —\n• You gain 1 life.\n• You gain 2 life.\nEntwine {2}",None).unwrap();
    for d in [&offer,&modes]{game::play::check(d).unwrap();}
    let mut g=Game::new(vec![offer,modes],2,20);g.put(0,1,Zone::Library);let card=g.put(0,0,Zone::Hand);mana(&mut g,2);
    g.act(0,Move::Cast{card,targets:vec![]}).unwrap();pass(&mut g);let card=g.exile[0];
    g.act(0,crate::support::paid("GG",modal(card,true,&[1,0],vec![]))).unwrap();cite!(["118.9d"]; assert_eq!(g.players[0].pool.total(),0));pass(&mut g);cite!(["702.42a"]; assert_eq!(g.players[0].life,23));
}

#[test]
fn r608_2b_one_illegal_modal_target_keeps_the_other_mode() {
    let mut g=setup(&["Barbed Lightning","Craw Wurm"]);let wurm=g.put(1,1,Zone::Battlefield);let card=g.put(0,0,Zone::Hand);
    g.players[0].pool.colors[ir::Color::Red as usize]=5;
    let before=format!("{g:?}");crate::support::refused(g.act(0,modal(card,true,&[0,1],vec![vec![Target::Object(wurm)]])), "that has 2 target group(s), 1 given");cite!(["702.42a","700.2c"]; refused("wrong number of targets: 0 chosen, it takes 1 (601.2c)"): g.act(0,modal(card,true,&[0,1],vec![vec![Target::Object(wurm)],vec![]])));assert_eq!(format!("{g:?}"),before);
    g.act(0,crate::support::paid("RRRRR",modal(card,true,&[0,1],vec![vec![Target::Object(wurm)],vec![Target::Player(1)]]))).unwrap();
    g.move_object(wurm,Zone::Exile);pass(&mut g);cite!(["608.2b"]; assert_eq!(g.players[1].life,17));assert!(g.stack.is_empty());
}
#[test]
fn r601_2b_foreign_entwine_flag_is_refused_even_when_false() {
    let mut g=setup(&["Lightning Bolt"]);let card=g.put(0,0,Zone::Hand);g.players[0].pool.colors[ir::Color::Red as usize]=1;
    for paid in [false,true]{let before=format!("{g:?}");cite!(["601.2b"]; refused("that spell has no entwine cost (702.42)"): g.act(0,Move::WithEntwine(paid,Box::new(Move::Cast{card,targets:vec![vec![Target::Player(1)]]}))));assert_eq!(format!("{g:?}"),before);}
}
