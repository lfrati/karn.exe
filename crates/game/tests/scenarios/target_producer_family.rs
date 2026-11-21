//! Explicit declarations use the same announcement and retained target semantics
//! as target operands: CR 115.1a/d, 601.2c, 602.2b, 603.3d, 608.2b-d, 400.7.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
fn card(kind: &str, text: &str) -> CardDef {
    let d = CardDef::new("Declaration", "{1}", kind, text, None).unwrap();
    game::play::check(&d).unwrap(); d
}
fn setup(d: CardDef) -> Game {
    Game::new(vec![d,
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 3))).unwrap(),
        card("Instant", "Return target creature to its owner's hand."),
        card("Instant", "Gain control of target creature."),
        CardDef::new("Rock", "{0}", "Artifact", "", None).unwrap()], 2, 20)
}
fn pass(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn cast(g: &mut Game, def: usize, targets: Vec<Vec<Target>>, spending: &str) {
    let card = g.put(0, def, Zone::Hand); g.players[0].pool.colorless += 1;
    g.act(0, paid(spending, Move::Cast { card, targets })).unwrap();
}
fn one(o: ObjId) -> Vec<Vec<Target>> { vec![vec![o.into()]] }

#[test]
fn r601_optional_zero_is_distinct_from_all_chosen_targets_illegal() {
    for choose in [false, true] {
        let mut g = setup(card("Instant", "Choose up to one target creature. Tap that creature. You gain 2 life."));
        let target = g.put(1, 1, Zone::Battlefield);
        cast(&mut g, 0, if choose { one(target) } else { vec![vec![]] }, "C");
        if choose { g.move_object(target, Zone::Graveyard); }
        pass(&mut g);
        cite!(["608.2b"]; assert_eq!(g.players[0].life, if choose {20} else {22}));
        cite!(["608.2b"]; assert_eq!(g.log.iter().any(|e| matches!(e, game::Event::Fizzled {..})), choose));
        assert!(g.stack.is_empty() && g.decision.is_none());
    }
}

#[test]
fn r601_plural_legality_payment_and_original_incarnations() {
    let mut g = setup(card("Instant", "Choose two target creatures. Tap those creatures. You gain 2 life."));
    let a = g.put(1, 1, Zone::Battlefield); let b = g.put(1, 1, Zone::Battlefield);
    let rock = g.put(1, 4, Zone::Battlefield); let spell = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colorless = 1;
    for (targets,reason) in [(vec![vec![a.into()]], "wrong number of targets: 1 chosen, it takes 2"), (vec![vec![a.into(), a.into()]], "the same target twice for one"), (vec![vec![a.into(), rock.into()]], "an illegal target, Rock")] {
        let before = format!("{g:?}");
        cite!(["115.3", "601.2c"]; refused(reason): g.act(0, paid("C", Move::Cast {card: spell, targets})));
        assert_eq!(format!("{g:?}"), before);
    }
    g.act(0, paid("C", Move::Cast {card: spell, targets: vec![vec![a.into(), b.into()]]})).unwrap();
    cite!(["601.2c", "601.2h"]; assert_eq!(g.players[0].pool.total(), 0));
    cast(&mut g, 2, one(a), "C"); pass(&mut g);
    let old_card = g.players[1].hand[0]; g.move_object(old_card, Zone::Battlefield);
    let returned = *g.battlefield.last().unwrap();
    cite!(["400.7"]; assert_ne!(a, returned));
    pass(&mut g);
    cite!(["608.2b", "400.7"]; assert!(!g.obj(returned).tapped && g.obj(b).tapped));
    cite!(["608.2b", "608.2c"]; assert_eq!(g.players[0].life, 22));
}

#[test]
fn r608_control_filter_rechecked_without_incarnation_change() {
    let mut g = setup(card("Instant", "Choose up to two target creatures you don't control. Tap those creatures. You gain 2 life."));
    let a = g.put(1, 1, Zone::Battlefield); let b = g.put(1, 1, Zone::Battlefield);
    cast(&mut g, 0, vec![vec![a.into(), b.into()]], "C");
    cast(&mut g, 3, one(a), "C"); pass(&mut g); pass(&mut g);
    cite!(["608.2b"]; assert!(!g.obj(a).tapped && g.obj(b).tapped));
    cite!(["608.2b", "608.2c"]; assert_eq!(g.players[0].life, 22));
}

#[test]
fn r602_activated_artifact_domain_and_retained_suspension() {
    use game::state::Answer;
    use game::program::Value;
    let mut g = setup(card("Artifact", "{1}: Choose up to one target artifact. Choose a color. Tap that artifact."));
    let source = g.put(0, 0, Zone::Battlefield); let target = g.put(1, 4, Zone::Battlefield);
    let bear = g.put(1, 1, Zone::Battlefield); g.players[0].pool.colorless = 1;
    let before = format!("{g:?}");
    cite!(["602.2b", "601.2c"]; refused("an illegal target, Bear"): g.act(0, paid("C", Move::Activate {source, ability:0, targets:one(bear), paying:vec![]})));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, paid("C", Move::Activate {source, ability:0, targets:one(target), paying:vec![]})).unwrap();
    pass(&mut g);
    cite!(["608.2d"]; assert!(matches!(g.decision, Some(game::state::Decision::Choose {player:0,..}))));
    g.act(0, Move::Choose(Answer::Value(Value::Color(ir::Color::Blue)))).unwrap();
    cite!(["608.2c", "608.2d"]; assert!(g.obj(target).tapped && !g.obj(source).tapped && !g.obj(bear).tapped));
    assert!(g.stack.is_empty() && g.decision.is_none());
}

#[test]
fn r603_triggered_graveyard_targets_and_announcement() {
    let mut g = setup(card("Artifact", "When this artifact enters, choose up to one target creature card in your graveyard. Return that card to your hand."));
    let target = g.put(0, 1, Zone::Graveyard); let other = g.put(1, 1, Zone::Graveyard);
    cast(&mut g, 0, vec![], "C"); pass(&mut g);
    cite!(["603.3d"]; assert!(matches!(g.decision, Some(game::state::Decision::Targets { player:0,..}))));
    let before = format!("{g:?}");
    cite!(["601.2c", "603.3d"]; refused("an illegal target, Bear"): g.act(0, Move::ChooseTargets(one(other))));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::ChooseTargets(one(target))).unwrap();
    cite!(["603.3d"]; assert!(g.players[0].hand.is_empty()));
    pass(&mut g);
    cite!(["608.2c", "400.7"]; assert_eq!(g.players[0].hand.len(), 1));
    assert!(g.players[1].graveyard.contains(&other));
}

#[test]
fn r601_dynamic_x_count_is_known_at_announcement() {
    let mut g = setup(CardDef::new("Variable", "{X}", "Instant", "Choose up to X target creatures. Tap those creatures.", None).unwrap());
    let a = g.put(1, 1, Zone::Battlefield); let b = g.put(1, 1, Zone::Battlefield);
    let spell = g.put(0, 0, Zone::Hand); g.players[0].pool.colorless = 1;
    let before = format!("{g:?}");
    cite!(["601.2b", "601.2c"]; refused("wrong number of targets: 2 chosen, it takes 0 to 1"): g.act(0, Move::WithX(1, Box::new(paid("C", Move::Cast {card:spell, targets:vec![vec![a.into(),b.into()]]})))));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::WithX(1, Box::new(paid("C", Move::Cast {card:spell, targets:one(a)})))).unwrap(); pass(&mut g);
    cite!(["601.2c", "608.2c"]; assert!(g.obj(a).tapped && !g.obj(b).tapped));
}

#[test]
fn r115_multiple_declarations_bind_each_group_without_retargeting() {
    let mut g = setup(card("Instant", "Choose target creature you control and target creature an opponent controls. Tap the creature you control. Untap the creature an opponent controls."));
    let mine = g.put(0, 1, Zone::Battlefield); let theirs = g.put(1, 1, Zone::Battlefield);
    g.set_tapped(theirs, true);
    let spell = g.put(0, 0, Zone::Hand); g.players[0].pool.colorless = 1;
    let before = format!("{g:?}");
    cite!(["601.2c"]; refused("an illegal target, Bear"): g.act(0, paid("C", Move::Cast {card:spell, targets:vec![vec![theirs.into()],vec![mine.into()]]})));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, paid("C", Move::Cast {card:spell, targets:vec![vec![mine.into()],vec![theirs.into()]]})).unwrap(); pass(&mut g);
    cite!(["608.2c", "608.2d"]; assert!(g.obj(mine).tapped && !g.obj(theirs).tapped && g.decision.is_none()));
}

#[test]
fn r115_player_and_stack_domains_share_declaration_lowering() {
    let mut g = setup(card("Instant", "Choose any number of target opponents. Those players lose 2 life."));
    cast(&mut g, 0, vec![vec![Target::Player(1)]], "C"); pass(&mut g);
    cite!(["115.1a", "608.2c"]; assert_eq!((g.players[0].life, g.players[1].life), (20,18)));
    let mut g = setup(card("Instant", "Choose target spell. Counter that spell."));
    let target = g.put(1, 2, Zone::Stack);
    cast(&mut g, 0, one(target), "C"); pass(&mut g);
    cite!(["115.1a", "701.6a"]; assert!(!g.stack.contains(&target) && g.players[1].graveyard.len() == 1));
}

fn native(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    CardDef::from_faces(&CardDef::catalog(&faces)[name]).unwrap()
}

#[test]
fn r608_native_beastie_beatdown_keeps_groups_and_requires_both_creatures() {
    // Pinned rulings 2711c8ac5b7bd59c and 701db72f49f49f0a: both
    // targets are required at announcement; either illegal prevents the damage.
    for depart in [false, true] {
        let d = native("Beastie Beatdown"); game::play::check(&d).unwrap();
        let mut g = setup(d);
        let mine = g.put(0, 1, Zone::Battlefield); let theirs = g.put(1, 1, Zone::Battlefield);
        g.players[0].pool.colors = [10;5];
        let spell = g.put(0, 0, Zone::Hand); let before = format!("{g:?}");
        cite!(["601.2c"]; refused("wrong number of targets: 0 chosen, it takes 1"): g.act(0, paid("RG", Move::Cast {card:spell, targets:vec![vec![mine.into()],vec![]]})));
        assert_eq!(format!("{g:?}"), before);
        g.act(0, paid("RG", Move::Cast {card:spell, targets:vec![vec![mine.into()],vec![theirs.into()]]})).unwrap();
        if depart { g.move_object(theirs, Zone::Graveyard); }
        pass(&mut g);
        cite!(["608.2b", "120.3e"]; assert_eq!(g.obj(mine).damage, 0));
        if !depart { cite!(["120.3e"]; assert_eq!(g.obj(theirs).damage,2)); }
        let dealt:Vec<_>=g.log.iter().filter_map(|e|match e {game::Event::DamageDealt{source,to,amount,..} if *source==mine=>Some((*to,*amount)),_=>None}).collect();
        cite!(["608.2b", "120.3e"]; assert_eq!(dealt,if depart {vec![]}else{vec![(theirs.into(),2)]}));
        // No delirium: the graveyard has fewer than four card types.
        assert!(g.obj(mine).counters.is_empty() && g.decision.is_none());
    }
}

#[test]
fn r602_native_living_library_announces_before_sacrifice_and_respects_owner() {
    let d = native("Living Library"); game::play::check(&d).unwrap();
    let mut g = setup(d); let source = g.put(0, 0, Zone::Battlefield);
    let mine = g.put(0, 1, Zone::Battlefield); let theirs = g.put(1, 1, Zone::Battlefield);
    g.players[0].pool.colorless = 6; let before = format!("{g:?}");
    cite!(["602.2b", "601.2c"]; refused("an illegal target, Bear"): g.act(0, paid("CCCCCC", Move::Activate {source, ability:0, targets:one(mine), paying:vec![]})));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, paid("CCCCCC", Move::Activate {source, ability:0, targets:one(theirs), paying:vec![]})).unwrap();
    cite!(["602.2b", "601.2h"]; assert!(!g.battlefield.contains(&source) && g.players[0].pool.total()==0));
    cite!(["601.2h", "608.2c"]; assert!(g.battlefield.contains(&theirs) && g.players[1].library.is_empty()));
    pass(&mut g);
    cite!(["400.3", "400.7", "608.2c"]; assert_eq!(g.players[1].library.len(),1));
    assert!(g.battlefield.contains(&mine) && !g.battlefield.contains(&theirs) && g.stack.is_empty());
}

#[test]
fn r608_chosen_declaration_is_not_an_intervening_created_result() {
    for choose in [false,true] {
        let mut g = setup(card("Instant", "Choose up to one target artifact. Create a Treasure token. Tap the chosen artifact."));
        let target = g.put(1, 4, Zone::Battlefield);
        cast(&mut g, 0, if choose { one(target) } else {vec![vec![]]}, "C"); pass(&mut g);
        cite!(["608.2c", "608.2d"]; assert_eq!(g.obj(target).tapped, choose));
        assert_eq!(g.battlefield.len(),2);
        let treasure = *g.battlefield.iter().find(|o| **o != target).unwrap();
        cite!(["608.2c"]; assert!(!g.obj(treasure).tapped));
    }
    let d = CardDef::new("Ambiguous", "{0}", "Instant", "Choose target artifact and target artifact. Tap the chosen artifact.", None).unwrap();
    // Both declarations fit. The compiler must refuse an ambiguous antecedent.
    assert_eq!(game::play::check(&d).unwrap_err().0, "chosen reference: ambiguous explicit choices");
}

#[test]
fn r603_native_daretti_targets_before_optional_sacrifice() {
    use game::state::{Answer, Choice, Decision};
    // Pinned ruling 54989e042536ba4c: announce the graveyard target when
    // the trigger is stacked; choose sacrifice and its object on resolution.
    for pay in [false,true] {
        let d = native("Daretti, Rocketeer Engineer"); game::play::check(&d).unwrap();
        let mut g = setup(d); let target = g.put(0, 4, Zone::Graveyard);
        let payment = g.put(0, 4, Zone::Battlefield); let spare = g.put(0, 4, Zone::Battlefield);
        g.players[0].pool.colors = [10;5]; cast(&mut g,0,vec![], "WWWWR"); pass(&mut g);
        cite!(["603.3d"]; assert!(matches!(g.decision,Some(Decision::Targets {player:0,..}))));
        assert!(g.battlefield.contains(&payment));
        g.act(0,Move::ChooseTargets(one(target))).unwrap(); pass(&mut g);
        cite!(["608.2d"]; assert!(matches!(g.decision,Some(Decision::Choose {player:0,choice:Choice::May {..}}))));
        g.act(0,Move::Choose(if pay {Answer::Yes} else {Answer::No})).unwrap();
        if pay {
            g.act(0,Move::Choose(Answer::Objects(vec![payment]))).unwrap();
            cite!(["608.2c", "400.7"]; assert!(!g.battlefield.contains(&payment) && !g.players[0].graveyard.contains(&target)));
            // The chosen target returns; the sacrificed artifact cannot steal its reference.
            assert_eq!(g.battlefield.len(),3);
        } else { assert!(g.players[0].graveyard.contains(&target) && g.battlefield.contains(&payment)); }
        assert!(g.battlefield.contains(&spare) && g.decision.is_none() && g.stack.is_empty());
    }
}

#[test]
fn r608_chosen_noun_cannot_steal_an_incompatible_explicit_choice() {
    use game::state::Answer;
    let mut g = setup(card("Instant", "Choose target artifact. Choose a creature you control. Tap the chosen creature."));
    let artifact = g.put(0,4,Zone::Battlefield);
    let creature = g.put(0,1,Zone::Battlefield); let spare = g.put(0,1,Zone::Battlefield);
    cast(&mut g,0,one(artifact), "C"); pass(&mut g);
    g.act(0,Move::Choose(Answer::Objects(vec![creature]))).unwrap();
    cite!(["608.2c", "608.2d"]; assert!(g.obj(creature).tapped && !g.obj(spare).tapped && !g.obj(artifact).tapped));
    let bad = CardDef::new("Mismatched", "{0}", "Instant", "Choose target artifact. Tap the chosen creature.", None).unwrap();
    crate::support::refused(game::play::check(&bad).map_err(|e|e.0),"input reference: nothing before it supplies the required participant/cardinality");
    let ambiguous = CardDef::new("Ambiguous", "{0}", "Instant", "Choose target artifact. Choose an artifact you control. Tap the chosen artifact.", None).unwrap();
    assert_eq!(game::play::check(&ambiguous).unwrap_err().0,"chosen reference: ambiguous explicit choices");
}
