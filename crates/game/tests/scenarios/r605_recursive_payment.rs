//! Authored mana windows nest at the activation that pays a mana cost
//! (601.2g–h, 602.2b, 605.3a–b); an individual resource is spent once (118.10).
use crate::support::{cite, mana_pool, paid};
use game::{CardDef, Event, Game, Move, ObjId, Zone};
use game::legal::Step;

fn step(source:ObjId)->Step {Step {source,ability:0,plan:vec![],x:None,life:vec![],paying:vec![],with:None,adding:vec![],chose:vec![],cost_branches:None}}
fn setup(filter_cost:&str, filter_text:&str, spell_cost:&str)->(Game,ObjId,ObjId,ObjId) {
    let defs=vec![
        CardDef::new("Forest","","Basic Land — Forest","",None).unwrap(),
        CardDef::new("Filter","{0}","Artifact",&format!("{filter_cost}: {filter_text}"),None).unwrap(),
        CardDef::new("Spell",spell_cost,"Sorcery","You gain 1 life.",None).unwrap(),
    ];
    for def in &defs {game::play::check(def).unwrap();}
    let mut game=Game::new(defs,2,20);
    let forest=game.put(0,0,Zone::Battlefield);let filter=game.put(0,1,Zone::Battlefield);let card=game.put(0,2,Zone::Hand);
    (game,forest,filter,card)
}
fn cast(card:ObjId, plan:Vec<Step>, mana:&str)->Move {paid(mana,Move::WithMana(plan,Box::new(Move::Cast{card,targets:vec![]})))}

#[test]
fn r605_3a_recursive_window_spends_each_cost_before_its_production() {
    let (mut g,forest,filter,card)=setup("{1}, {T}","Add {W}.","{W}");
    let child=Step {plan:vec![step(forest)],with:Some(mana_pool("G")),..step(filter)};
    g.begin_payment_audit();
    cite!(["601.2g","602.2b","605.3a","605.3b"];ok:g.act(0,cast(card,vec![child],"W")));
    assert!(g.obj(forest).tapped && g.obj(filter).tapped);
    assert_eq!(g.players[0].pool.total(),0);
    assert_eq!(g.stack.len(),1);
    let receipts=g.payment_audit();
    assert_eq!(receipts.iter().map(|r|r["path"].clone()).collect::<Vec<_>>(),vec![serde_json::json!([0,0]),serde_json::json!([0]),serde_json::json!([])]);
    assert_eq!(receipts[1]["prior_pool"]["G"],0);
    assert_eq!(receipts[1]["selected"]["G"],1);
    assert_eq!(receipts[2]["selected"]["W"],1);
    let production=g.payment_production_audit();
    assert_eq!(production.iter().map(|r|r["path"].clone()).collect::<Vec<_>>(),vec![serde_json::json!([0,0]),serde_json::json!([0])]);
    assert!(production.iter().all(|r|r["triggered"]==false));
}

#[test]
fn r118_10_invalid_deep_child_reverses_the_entire_unexposed_announcement() {
    let (mut g,forest,filter,card)=setup("{1}, {T}","Add {W}.","{W}");
    let child=Step {plan:vec![step(forest),step(forest)],with:Some(mana_pool("G")),..step(filter)};
    let before=format!("{g:?}");
    // The first activation taps Forest; the second cannot pay its {T} cost.
    cite!(["107.5","118.3","118.10","733.1"];refused("Forest can't pay {T} as it is (107.5, 118.3)"):g.act(0,cast(card,vec![child],"W")));
    assert_eq!(format!("{g:?}"),before);
    // Removing only the repeated activation makes this same announcement legal.
    let child=Step {plan:vec![step(forest)],with:Some(mana_pool("G")),..step(filter)};
    cite!(["602.2b","605.3b"];ok:g.act(0,cast(card,vec![child],"W")));
    assert!(g.obj(forest).tapped && g.obj(filter).tapped);
    assert_eq!(g.stack.len(),1);
    assert_eq!(g.players[0].pool.total(),0);
}

#[test]
fn r605_3b_repeated_source_receipts_name_distinct_activation_scopes() {
    let (mut g,forest,filter,card)=setup("{1}","Add {C}{C}.","{3}");
    let first=Step {plan:vec![step(forest)],with:Some(mana_pool("G")),..step(filter)};
    let second=Step {with:Some(mana_pool("C")),..step(filter)};
    g.begin_payment_audit();
    cite!(["605.3b","118.10"];ok:g.act(0,cast(card,vec![first,second],"CCC")));
    assert_eq!(g.players[0].pool.total(),0);
    let production=g.payment_production_audit();
    assert_eq!(production[1]["source"],production[2]["source"]);
    assert_eq!(production[1]["path"],serde_json::json!([0]));
    assert_eq!(production[2]["path"],serde_json::json!([1]));
    assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Activated{source,..} if *source==filter)).count(),2);
}

#[test]
fn r107_3a_child_x_and_phyrexian_life_are_announced_for_that_activation() {
    let (mut g,forest,filter,card)=setup("{X}{G/P}, {T}","Add {G}.","{G}");
    let child=Step {plan:vec![step(forest)],x:Some(1),life:vec![true],with:Some(mana_pool("G")),..step(filter)};
    g.begin_payment_audit();
    cite!(["107.3a","118.13a","602.2b"];ok:g.act(0,cast(card,vec![child],"G")));
    assert_eq!(g.players[0].life,18);
    assert_eq!(g.players[0].pool.total(),0);
    let receipts=g.payment_audit();
    assert_eq!(receipts[1]["requirement"]["symbols"],serde_json::json!(["1"]));
    assert_eq!(receipts[1]["symbol_indices"],serde_json::json!([0]));
    assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Activated{..})).count(),2);
}

#[test]
fn r605_3c_same_active_mana_ability_cannot_recur_even_with_sufficient_explicit_resources() {
    let (mut g,_forest,filter,card)=setup("{1}","Add {C}{C}.","{3}");
    g.players[0].pool=mana_pool("CC");
    let inner=Step{with:Some(mana_pool("C")),..step(filter)};
    let outer=Step{with:Some(mana_pool("C")),plan:vec![inner],..step(filter)};
    let before=format!("{g:?}");
    cite!(["605.3c"];refused("that mana ability is already being activated or resolving (605.3c)"):g.act(0,cast(card,vec![outer],"CCC")));
    assert_eq!(format!("{g:?}"),before);
    // A refused reservation must not block the next legal use of that key.
    g.act(0,paid("C",Move::Activate{source:filter,ability:0,targets:vec![],paying:vec![]})).unwrap();
    assert_eq!(g.players[0].pool.total(),3);
}

#[test]
fn r605_3c_different_mana_abilities_on_the_same_source_may_nest() {
    let defs=vec![CardDef::new("Prism","{0}","Artifact","{1}: Add {C}{C}.\n{1}: Add {W}.",None).unwrap(),
        CardDef::new("Spell","{1}{W}","Sorcery","You gain 1 life.",None).unwrap()];
    let mut g=Game::new(defs,2,20);
    let source=g.put(0,0,Zone::Battlefield);let card=g.put(0,1,Zone::Hand);
    g.players[0].pool=mana_pool("C");
    let inner=Step{with:Some(mana_pool("C")),..step(source)};
    let outer=Step{ability:1,with:Some(mana_pool("C")),plan:vec![inner],..step(source)};
    cite!(["605.3a","605.3c"];ok:g.act(0,cast(card,vec![outer],"CW")));
    assert_eq!(g.players[0].pool.total(),0);
    assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Activated{..})).count(),2);
}

#[test]
fn r605_3c_reentry_is_refused_while_mana_resolution_asks_for_an_effect_payment() {
    use game::state::{Answer,Choice,Decision};
    let defs=vec![CardDef::new("Prism","{0}","Artifact","{1}: You may pay {1}. If you do, add {G}.",None).unwrap()];
    game::play::check(&defs[0]).unwrap();
    let mut g=Game::new(defs,2,20);let source=g.put(0,0,Zone::Battlefield);
    g.players[0].pool=mana_pool("CCC");
    g.act(0,paid("C",Move::Activate{source,ability:0,targets:vec![],paying:vec![]})).unwrap();
    assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::Pay{..},..})));
    let before=format!("{g:?}");
    let child=Step{with:Some(mana_pool("C")),..step(source)};
    cite!(["605.3c"];refused("that mana ability is already being activated or resolving (605.3c)"):g.act(0,Move::Choose(Answer::Pay{plan:vec![child],with:Some(mana_pool("C")),tapped:vec![]})));
    assert_eq!(format!("{g:?}"),before);
    let mut clone=g.clone();
    for game in [&mut g,&mut clone] {
        game.act(0,Move::Choose(Answer::Pay{plan:vec![],with:Some(mana_pool("C")),tapped:vec![]})).unwrap();
        assert_eq!(game.players[0].pool,mana_pool("CG"));
        game.act(0,paid("C",Move::Activate{source,ability:0,targets:vec![],paying:vec![]})).unwrap();
        assert!(matches!(game.decision,Some(Decision::Choose{choice:Choice::Pay{..},..})));
    }
}
