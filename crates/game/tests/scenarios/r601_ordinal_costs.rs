//! Ordinal cost changes count completed matching casts before 601.2f, never announcements.
use crate::support::{cite,paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};
use std::sync::OnceLock;
fn leveilleurs() -> Vec<CardDef> {
    static DEFS: OnceLock<Vec<CardDef>>=OnceLock::new();
    DEFS.get_or_init(|| {
        let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        ["Alisaie Leveilleur","Alphinaud Leveilleur"].iter().map(|name| {
            let d=CardDef::from_face(faces.iter().find(|f|f.face_name==*name).unwrap()).unwrap();game::play::check(&d).unwrap();d
        }).collect()
    }).clone()
}
fn def(name: &str, cost: &str, kind: &str, text: &str, pt: Option<(i32,i32)>) -> CardDef {
    let d=CardDef::new(name,cost,kind,text,pt).unwrap();game::play::check(&d).unwrap();d
}
fn fixture() -> Game {
    let mut defs=leveilleurs();defs.extend([
        def("Spell","{3}","Instant","You gain 1 life.",None),
        def("Free","{0}","Instant","You gain 1 life.",None),
        def("Island","","Basic Land — Island","",None),
        def("Resource","{0}","Artifact","Sacrifice this artifact: Add {C}.",None),
        def("Bear","{3}","Creature","",Some((2,2))),
    ]);
    let mut g=Game::new(defs,2,20);g.part=ir::Part::PrecombatMain;
    for p in 0..2 {for _ in 0..10 {g.put(p,4,Zone::Library);}}g
}
fn cast(g: &mut Game, player: usize, index: usize, units: &str) -> ObjId {
    let card=g.put(player,index,Zone::Hand);g.act(player,paid(units,Move::Cast{card,targets:vec![]})).unwrap();card
}
fn pass(g: &mut Game) {for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}}
fn settle(g: &mut Game) {
    for _ in 0..30 {
        match g.decision.clone() {
            Some(Decision::Targets{player,..})=>g.act(player,Move::ChooseTargets(vec![vec![Target::Player(player)]])).unwrap(),
            Some(Decision::Choose{player,choice:Choice::May})=>g.act(player,Move::Choose(Answer::No)).unwrap(),
            None if g.stack.is_empty()=>return,
            None=>pass(g),other=>panic!("unexpected: {other:?}"),
        }
    }panic!("did not settle");
}


#[test]
fn r601_2f_full_alisaie_only_discounts_the_second_completed_cast_and_failed_payment_does_not_count() {
    let mut g=fixture();g.put(0,0,Zone::Battlefield);
    let first=g.put(0,2,Zone::Hand);g.players[0].pool.colorless=1;
    // the whole reason, equal: a substring of it would also match one naming a reduction
    assert!(!g.clone().act(g.priority, paid("CCC",Move::Cast { card: first, targets: vec![] })).is_ok());cite!(["601.2f"]; assert_eq!(g.act(0,paid("C",Move::Cast{card:first,targets:vec![]})), Err("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)".into())));
    g.players[0].pool.colorless=3;assert!(g.clone().act(g.priority, paid("CCC",Move::Cast { card: first, targets: vec![] })).is_ok());g.act(0,paid("CCC",Move::Cast{card:first,targets:vec![]})).unwrap();
    cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(),0));settle(&mut g);
    let second=g.put(0,2,Zone::Hand);assert!(!g.clone().act(g.priority, paid("C",Move::Cast { card: second, targets: vec![] })).is_ok());
    cite!(["601.2f"]; refused("your mana pool cannot pay that cost (118.3a): {1} in total, {2} less than it was (601.2f)"): g.act(0,Move::Cast{card:second,targets:vec![]}));
    g.players[0].pool.colorless=1;assert!(g.clone().act(g.priority, paid("C",Move::Cast { card: second, targets: vec![] })).is_ok());g.act(0,paid("C",Move::Cast{card:second,targets:vec![]})).unwrap();
    cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(),0));settle(&mut g);
    let third=g.put(0,2,Zone::Hand);g.players[0].pool.colorless=1;
    assert!(!g.clone().act(g.priority, paid("CCC",Move::Cast { card: third, targets: vec![] })).is_ok());cite!(["601.2f"]; assert_eq!(g.act(0,paid("C",Move::Cast{card:third,targets:vec![]})), Err("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)".into())));
    g.players[0].pool.colorless=3;g.act(0,paid("CCC",Move::Cast{card:third,targets:vec![]})).unwrap();
    cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(),0));settle(&mut g);assert_eq!(g.players[0].life,23);
}

#[test]
fn r601_2f_full_alisaie_counts_casts_before_entry_and_preserves_colored_mana() {
    let mut g=fixture();g.players[0].pool.colorless=3;g.players[0].pool.colors[ir::Color::White as usize]=1;
    cast(&mut g,0,0,"CCW");settle(&mut g);
    cite!(["601.2f"]; assert_eq!(g.players[0].pool.colorless,1)); // casting Alisaie itself was first
    cast(&mut g,0,6,"C");cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(),0));settle(&mut g);
    let mut g=fixture();cast(&mut g,0,3,"");settle(&mut g);g.put(0,0,Zone::Battlefield);
    let alphinaud=g.put(0,1,Zone::Hand);g.players[0].pool.colorless=4;
    assert!(!g.clone().act(g.priority, paid("CU",Move::Cast { card: alphinaud, targets: vec![] })).is_ok());cite!(["601.2f"]; refused("your mana pool cannot pay that cost (118.3a): {1}{U} in total, {2} less than it was (601.2f)"): g.act(0,Move::Cast{card:alphinaud,targets:vec![]}));
    g.players[0].pool.colorless=1;g.players[0].pool.colors[ir::Color::Blue as usize]=1;
    assert!(g.clone().act(g.priority, paid("CU",Move::Cast { card: alphinaud, targets: vec![] })).is_ok());g.act(0,paid("CU",Move::Cast{card:alphinaud,targets:vec![]})).unwrap();cite!(["601.2f", "118.7a"]; assert_eq!(g.players[0].pool.total(),0));settle(&mut g);
}

#[test]
fn r601_2f_other_players_casts_do_not_use_or_advance_the_controller_ordinal() {
    let mut g=fixture();g.put(0,0,Zone::Battlefield);
    g.act(0,Move::Pass).unwrap();cast(&mut g,1,3,"");settle(&mut g);
    let own=g.put(0,2,Zone::Hand);g.players[0].pool.colorless=1;assert!(!g.clone().act(g.priority, paid("C",Move::Cast { card: own, targets: vec![] })).is_ok());
    cast(&mut g,0,3,"");settle(&mut g);
    g.act(0,Move::Pass).unwrap();let other=g.put(1,2,Zone::Hand);g.players[1].pool.colorless=1;
    assert!(!g.clone().act(g.priority, paid("C",Move::Cast { card: other, targets: vec![] })).is_ok());cite!(["601.2f"]; assert_eq!(g.act(1,paid("C",Move::Cast{card:other,targets:vec![]})), Err("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)".into())));
    g.act(1,Move::Pass).unwrap();g.players[0].pool.colorless=1;assert!(g.clone().act(g.priority, paid("C",Move::Cast { card: own, targets: vec![] })).is_ok());g.act(0,paid("C",Move::Cast{card:own,targets:vec![]})).unwrap();cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(),0));
}

#[test]
fn r601_2f_ordinal_cost_is_locked_before_its_source_is_sacrificed_for_mana() {
    let source=def("Reducer","{0}","Artifact","The second spell you cast each turn costs {2} less to cast.\nSacrifice this artifact: Add {C}.",None);
    let mut g=fixture();std::sync::Arc::make_mut(&mut g.defs).push(source);let source=g.put(0,g.defs.len()-1,Zone::Battlefield);
    cast(&mut g,0,3,"");settle(&mut g);let card=g.put(0,2,Zone::Hand);
    let plan = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source, ability: 1, paying: vec![], adding: vec![], chose: vec![], cost_branches: None }];
    cite!(["601.2f", "601.2g"]; ok: g.act(0,Move::WithMana(plan,Box::new(paid("C",Move::Cast{card,targets:vec![]})))));
    assert!(!g.battlefield.contains(&source));assert_eq!(g.players[0].pool.total(),0);assert_eq!(g.stack.len(),1);
}

#[test]
fn r603_2_full_alphinaud_draws_for_the_second_cast_before_the_spell_resolves() {
    let mut g=fixture();cast(&mut g,0,3,"");settle(&mut g);g.put(0,1,Zone::Battlefield);
    cast(&mut g,0,3,"");assert_eq!(g.stack.len(),2);assert!(g.players[0].hand.is_empty());
    pass(&mut g);cite!(["603.3"]; assert_eq!(g.players[0].hand.len(),1));assert_eq!(g.players[0].life,21);assert_eq!(g.stack.len(),1);
    settle(&mut g);cast(&mut g,0,3,"");assert_eq!(g.stack.len(),1);settle(&mut g);cite!(["603.2"]; assert_eq!(g.players[0].hand.len(),1));
}

#[test]
fn r601_2f_filtered_ordinals_count_only_matching_completed_spells_and_reset_each_turn() {
    let reducer=def("Filtered","{0}","Artifact","The second noncreature spell you cast each turn costs {2} less to cast.",None);
    let mut g=fixture();std::sync::Arc::make_mut(&mut g.defs).push(reducer);g.put(0,g.defs.len()-1,Zone::Battlefield);
    g.players[0].pool.colorless=3;cast(&mut g,0,6,"CCC");settle(&mut g);
    let first=g.put(0,2,Zone::Hand);g.players[0].pool.colorless=1;assert!(!g.clone().act(g.priority, paid("C",Move::Cast { card: first, targets: vec![] })).is_ok());
    cast(&mut g,0,3,"");settle(&mut g);cite!(["601.2f"]; assert!(g.clone().act(g.priority, paid("C",Move::Cast { card: first, targets: vec![] })).is_ok()));
    g.act(0,paid("C",Move::Cast{card:first,targets:vec![]})).unwrap();settle(&mut g);
    let third=g.put(0,2,Zone::Hand);g.players[0].pool.colorless=1;assert!(!g.clone().act(g.priority, paid("C",Move::Cast { card: third, targets: vec![] })).is_ok());
    let turn=g.turn;
    for _ in 0..100 {if g.turn>turn {break;}match g.decision.clone(){
        Some(Decision::Attackers{player})=>g.act(player,Move::Attack(vec![])).unwrap(),
        Some(Decision::Blockers{player})=>g.act(player,Move::Block(vec![])).unwrap(),
        None=>g.act(g.priority,Move::Pass).unwrap(),other=>panic!("{other:?}"),
    }}
    assert!(g.turn>turn);g.players[0].pool.colorless=1;cite!(["601.2f"]; assert!(!g.clone().act(g.priority, paid("C",Move::Cast { card: third, targets: vec![] })).is_ok()));
}

#[test]
fn r733_1_reversed_suspended_announcement_does_not_consume_the_second_spell_discount() {
    use game::legal::Step;
    use ir::mana::ManaSymbol;
    let mut g=fixture();
    std::sync::Arc::make_mut(&mut g.defs).extend([
        def("Mana creature","{0}","Creature — Elf","{T}: Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.",Some((1,1))),
        def("Hardened Scales","{G}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",None),
        def("Branching Evolution","{2}{G}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.",None),
        def("Expensive","{6}","Instant","You gain 1 life.",None),
    ]);
    g.put(0,0,Zone::Battlefield);let source=g.put(0,7,Zone::Battlefield);g.put(0,8,Zone::Battlefield);let evolution=g.put(0,9,Zone::Battlefield);
    cast(&mut g,0,3,"");settle(&mut g);let card=g.put(0,10,Zone::Hand);
    g.act(0,Move::WithMana(vec![Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source,ability:0,paying:vec![],adding:vec![vec![ManaSymbol::Color(ir::Color::Green);4]]}],Box::new(Move::Cast{card,targets:vec![]}))).unwrap();
    let Some(Decision::Choose{choice:Choice::Replacement{among,..},..})=&g.decision else {panic!("{:?}",g.decision)};
    let replacement=among.iter().find(|r|r.source==evolution).unwrap().clone();
    g.act(0,Move::Choose(Answer::Replacement(replacement))).unwrap();
    if matches!(g.decision,Some(Decision::Choose{choice:Choice::Mana{..},..})) {
        g.act(0,Move::Choose(Answer::Mana(vec![ManaSymbol::Color(ir::Color::Green);3]))).unwrap();
    }
    assert!(matches!(g.decision,Some(Decision::Payment{player:0,..})),"{:?}",g.decision);assert_eq!(g.players[0].pool.total(),3);
    let clone=g.clone();for mut branch in [g,clone] {
        branch.act(0,Move::ReversePayment).unwrap();assert_eq!(branch.obj(card).zone,Zone::Hand);
        branch.players[0].pool.colorless=1; // total 4, exactly the still-second cast's cost
        assert!(branch.clone().act(branch.priority, paid("CGGG",Move::Cast { card: card, targets: vec![] })).is_ok());branch.act(0,paid("CGGG",Move::Cast{card,targets:vec![]})).unwrap();
        cite!(["733.1", "601.2f"]; assert_eq!(branch.players[0].pool.total(),0));assert_eq!(branch.obj(source).counters.get("+1/+1"),Some(&3));
        let next=branch.put(0,2,Zone::Hand);branch.players[0].pool.colorless=1;assert!(!branch.clone().act(branch.priority, paid("C",Move::Cast { card: next, targets: vec![] })).is_ok());
    }
}
