//! Which object "each other creature that player
//! controls" is other than, when "that player" came from an earlier target. Run with
//! `engine::other_target_reference::`.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

fn pinned(name: &str) -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    CardDef::from_faces(CardDef::catalog(&faces).get(name).unwrap()).unwrap()
}
fn admitted(d: &CardDef) -> Result<(), String> { game::play::check(d).map_err(|e| e.0) }
fn custom(name: &str, cost: &str, ty: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef { CardDef::new(name, cost, ty, text, pt).unwrap() }
fn creature(name: &str, p: i32, t: i32, text: &str) -> CardDef { custom(name, "{0}", "Creature — Beast", text, Some((p, t))) }
fn pass(g: &mut Game) { for _ in 0..3 { if g.decision.is_some() || g.stack.is_empty() { return; } g.act(g.priority, Move::Pass).unwrap(); } }
fn cast(g: &mut Game, card: usize, targets: Vec<Vec<Target>>, spending: &str) {
    g.players[0].pool.colors = [10; 5];
    let c = g.put(0, card, Zone::Hand);
    g.act(0, crate::support::paid(spending, Move::Cast { card: c, targets })).unwrap();
    pass(g);
}
fn damage(g: &Game) -> Vec<(ObjId, Target, u32)> {
    g.log.iter().filter_map(|e| if let Event::DamageDealt { source, to, amount, .. } = e { Some((*source, *to, *amount)) } else { None }).collect()
}
fn to(g: &Game, o: ObjId) -> u32 { damage(g).iter().filter(|(_, t, _)| *t == Target::Object(o)).map(|(_, _, n)| n).sum() }
fn report(name: &str, d: &CardDef) -> bool {
    let r = admitted(d);
    println!("{name}: admitted={:?}", r);
    r.is_ok()
}

#[test]
fn public_execution_surviving_target_is_not_another_creature() {
    let pe = pinned("Public Execution");
    assert!(report("Public Execution", &pe));
    let mut g = Game::new(vec![pe, creature("Iron", 3, 5, "Indestructible"), creature("Bear", 2, 2, "")], 2, 20);
    let iron = g.put(1, 1, Zone::Battlefield);
    let bear = g.put(1, 2, Zone::Battlefield);
    cast(&mut g, 0, vec![vec![Target::Object(iron)]], "WWWWWB");
    println!("iron power={:?} bear power={:?}", g.power(iron), g.power(bear));
    cite!(["702.12b"]; assert_eq!(g.obj(iron).zone, Zone::Battlefield));
    // Uncited (card text): Public Execution's "Each other creature that player controls gets -2/-0": other than the surviving target
    assert_eq!((g.power(iron), g.power(bear)), (Some(3), Some(0)));
}

#[test]
fn provenance_target_illegal_other_target_legal() {
    let text = "Tap target creature an opponent controls and target creature you control. Each other creature that player controls gets -2/-0 until end of turn.";
    let d = custom("Probe Legal", "{W}", "Sorcery", text, None);
    assert!(report("Probe Legal", &d));
    let mut g = Game::new(vec![d, creature("Bear", 3, 3, "")], 2, 20);
    let theirs = g.put(1, 1, Zone::Battlefield);
    let their_other = g.put(1, 1, Zone::Battlefield);
    let mine = g.put(0, 1, Zone::Battlefield);
    let mine_other = g.put(0, 1, Zone::Battlefield);
    g.players[0].pool.colors = [10; 5];
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, crate::support::paid("W", Move::Cast { card: c, targets: vec![vec![Target::Object(theirs)], vec![Target::Object(mine)]] })).unwrap();
    g.move_object(theirs, Zone::Hand);
    pass(&mut g);
    println!("  mine tapped={} powers their_other={:?} mine={:?} mine_other={:?}", g.obj(mine).tapped, g.power(their_other), g.power(mine), g.power(mine_other));
    cite!(["608.2b"]; assert!(g.obj(mine).tapped));
    cite!(["608.2b"]; assert_eq!((g.power(their_other), g.power(mine), g.power(mine_other)), (Some(3), Some(3), Some(3))));
}

/// A distinct explicit actor and an earlier proven target: the two possible antecedents
/// are an unsupported ambiguous reference, not invalid Magic or union exclusions.
#[test]
fn distinct_explicit_actor_and_proven_target_are_refused() {
    for actor in ["Target creature you control", "Target creature"] {
        let text = format!("Tap target creature an opponent controls. {actor} deals damage equal to its power to each other creature that player controls.");
        let d = custom("Conflict", "{R}", "Sorcery", &text, None);
        let err = admitted(&d).expect_err("distinct actor and earlier target must not be guessed");
        assert!(err.contains("other: ambiguous earlier target and explicit actor"), "{err}");
    }
}

/// ControllerOf(target) replaces the source exclusion, including when controllers differ.
#[test]
fn direct_target_controller_excludes_target_and_hits_eligible_source() {
    for (text, first) in [
        ("{T}: This creature deals 3 damage to target creature and 1 damage to each other creature its controller controls.", 3),
        ("{T}: Tap target creature. Then this creature deals 1 damage to each other creature that creature's controller controls.", 0),
    ] {
        for owner in [0, 1] {
            let d = creature("Sparker", 1, 4, text);
            admitted(&d).unwrap();
            let mut g = Game::new(vec![d, creature("Bear", 2, 6, "")], 2, 20);
            let source = g.put(0, 0, Zone::Battlefield);
            let target = g.put(owner, 1, Zone::Battlefield);
            let other = g.put(owner, 1, Zone::Battlefield);
            let outside = g.put(1-owner, 1, Zone::Battlefield);
            g.act(0, Move::Activate { source, ability: 0, targets: vec![vec![Target::Object(target)]], paying: vec![] }).unwrap();
            let cloned = g.clone();
            for mut g in [g, cloned] {
                pass(&mut g);
                // Uncited (card text): "1 damage to each other creature its controller controls" / "that creature's controller controls": other than the target, among its controller's
                assert_eq!((to(&g, target), to(&g, other), to(&g, outside)), (first, 1, 0), "{text} target owner {owner}");
                assert_eq!(to(&g, source), if owner == 0 { 1 } else { 0 });
                assert!(damage(&g).iter().all(|(s, _, _)| *s == source));
            }
        }
    }
}

/// Flames of the Raze-Boar in full: exact amounts and condition/clone contrasts.
#[test]
fn flames_excludes_target_without_rebinding_condition_or_damage_source() {
    for (big, toughness) in [(true, 7), (false, 7), (true, 4)] {
        let d = pinned("Flames of the Raze-Boar"); admitted(&d).unwrap();
        let mut g = Game::new(vec![d, creature("Ox", 5, toughness, ""), creature("Wall", 1, 5, ""), creature("Big", 6, 6, "")], 2, 20);
        let target = g.put(1, 1, Zone::Battlefield);
        let other = g.put(1, 2, Zone::Battlefield);
        let mine = if big { g.put(0, 3, Zone::Battlefield) } else { g.put(0, 2, Zone::Battlefield) };
        g.players[0].pool.colors = [10; 5];
        let spell = g.put(0, 0, Zone::Hand);
        g.act(0, crate::support::paid("WWWWWR", Move::Cast { card: spell, targets: vec![vec![Target::Object(target)]] })).unwrap();
        let actor = g.stack[0];
        let cloned = g.clone();
        for mut g in [g, cloned] {
            pass(&mut g);
            // Flames' "4 damage to target creature ... Then ... 2 damage to each other creature that player controls if you control a creature with power 4 or greater": the power is read as it resolves (608.2h); the recipients are card text
            cite!(["608.2h"]; case: format!("big={big} toughness={toughness}"); assert_eq!((to(&g, target), to(&g, other), to(&g, mine)), (4, if big {2} else {0}, 0)));
            assert!(damage(&g).iter().all(|(s, _, _)| *s == actor));
            cite!(["704.5g"]; case: format!("big={big} toughness={toughness}"); assert_eq!(g.obj(target).zone, if toughness == 4 { Zone::Gone } else { Zone::Battlefield }));
        }
    }
}

#[test]
fn sole_illegal_target_prevents_flames_and_public_execution() {
    for name in ["Flames of the Raze-Boar", "Public Execution"] {
        let d = pinned(name); admitted(&d).unwrap();
        let mut g = Game::new(vec![d, creature("Bear", 3, 5, ""), creature("Big", 6, 6, "")], 2, 20);
        let target = g.put(1, 1, Zone::Battlefield);
        let other = g.put(1, 1, Zone::Battlefield);
        g.put(0, 2, Zone::Battlefield);
        g.players[0].pool.colors = [10; 5];
        let spell = g.put(0, 0, Zone::Hand);
        g.act(0, crate::support::paid(if name == "Flames of the Raze-Boar" { "WWWWWR" } else { "WWWWWB" }, Move::Cast { card: spell, targets: vec![vec![Target::Object(target)]] })).unwrap();
        g.move_object(target, Zone::Hand);
        pass(&mut g);
        cite!(["608.2b"]; case: name; assert!(damage(&g).is_empty()));
        cite!(["608.2b"]; case: name; assert_eq!(g.power(other), Some(3)));
    }
}

#[test]
fn a_set_without_other_still_includes_its_controller_producing_target() {
    let text = "Tap target creature an opponent controls. Each creature that player controls gets -2/-0 until end of turn.";
    let d = custom("All", "{B}", "Sorcery", text, None); admitted(&d).unwrap();
    let mut g = Game::new(vec![d, creature("Bear", 3, 5, "")], 2, 20);
    let target = g.put(1, 1, Zone::Battlefield);
    let other = g.put(1, 1, Zone::Battlefield);
    cast(&mut g, 0, vec![vec![Target::Object(target)]], "B");
    cite!(["613.4c"]; assert_eq!((g.power(target), g.power(other)), (Some(1), Some(1))));
}

#[test]
fn ambiguous_player_producers_do_not_use_the_last_target() {
    let text = "Tap target creature an opponent controls. Tap target creature an opponent controls. Each other creature that player controls gets -2/-0 until end of turn.";
    let d = custom("Ambiguous", "{B}", "Sorcery", text, None);
    crate::support::refused(admitted(&d), "reference: ambiguous earlier subject producers");
}

#[test]
fn same_explicit_actor_and_proven_target_keeps_one_exclusion() {
    let d = custom("One actor", "{R}", "Sorcery", "Target creature an opponent controls deals 1 damage to each other creature that player controls.", None);
    admitted(&d).unwrap();
    let mut g = Game::new(vec![d, creature("Bear", 3, 5, "")], 2, 20);
    let target = g.put(1, 1, Zone::Battlefield);
    let other = g.put(1, 1, Zone::Battlefield);
    cast(&mut g, 0, vec![vec![Target::Object(target)]], "R");
    // Uncited (card text): "Target creature an opponent controls deals 1 damage to each other creature that player controls"
    assert_eq!((to(&g, target), to(&g, other)), (0, 1));
}

#[test]
fn deferred_condition_can_read_the_targets_captured_player() {
    let text = "Tap target creature an opponent controls if that player is an opponent.";
    let d = custom("Conditional", "{B}", "Sorcery", text, None); admitted(&d).unwrap();
    for (illegal, expected) in [(false, true), (true, false)] {
        let mut g = Game::new(vec![d.clone(), creature("Bear", 3, 5, "")], 2, 20);

        let target = g.put(1, 1, Zone::Battlefield);
        g.players[0].pool.colors = [10; 5];
        let spell = g.put(0, 0, Zone::Hand);
        g.act(0, crate::support::paid("B", Move::Cast { card: spell, targets: vec![vec![Target::Object(target)]] })).unwrap();
        if illegal { g.move_object(target, Zone::Hand); }
        pass(&mut g);
        cite!(["608.2b"]; case: format!("illegal={illegal}"); assert_eq!(g.obj(target).tapped, expected));
    }
}

#[test]
fn public_execution_normal_target_dies_and_other_creatures_are_fixed_at_resolution() {
    let d = pinned("Public Execution"); admitted(&d).unwrap();
    let mut g = Game::new(vec![d, creature("Bear", 3, 5, "")], 2, 20);
    let target = g.put(1, 1, Zone::Battlefield);
    let other = g.put(1, 1, Zone::Battlefield);
    let mine = g.put(0, 1, Zone::Battlefield);
    cast(&mut g, 0, vec![vec![Target::Object(target)]], "WWWWWB");
    cite!(["701.8a"]; assert_eq!(g.obj(target).zone, Zone::Gone));
    cite!(["613.4c"]; assert_eq!((g.power(other), g.power(mine)), (Some(1), Some(3))));
    let late = g.put(1, 1, Zone::Battlefield);
    cite!(["611.2c"]; assert_eq!(g.power(late), Some(3), "611.2c: later creatures aren't in the retained set"));
}

#[test]
fn deputy_of_detention_keeps_explicit_name_comparison_precedence() {
    let d = pinned("Deputy of Detention"); admitted(&d).unwrap();
    let mut g = Game::new(vec![d, creature("Bear", 3, 5, ""), creature("Ox", 3, 5, "")], 2, 20);
    let target = g.put(1, 1, Zone::Battlefield);
    let same = g.put(1, 1, Zone::Battlefield);
    let mine = g.put(0, 1, Zone::Battlefield);
    let different = g.put(1, 2, Zone::Battlefield);
    cast(&mut g, 0, vec![], "WWU");
    cite!(["603.3d"]; assert!(matches!(g.decision, Some(game::state::Decision::Targets { .. }))));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(target)]])).unwrap();
    pass(&mut g);
    assert_eq!(g.exile.len(), 2);
    cite!(["201.2a"]; assert_eq!((g.obj(target).zone, g.obj(same).zone), (Zone::Gone, Zone::Gone)));
    assert_eq!((g.obj(mine).zone, g.obj(different).zone), (Zone::Battlefield, Zone::Battlefield));
}

#[test]
fn ordinary_controller_relations_without_target_provenance_keep_their_defaults() {
    for text in [
        "{T}: This creature deals 1 damage to each other creature you control.",
        "{T}: This creature deals 1 damage to each other creature an opponent controls.",
        "{T}: This creature deals 1 damage to each other creature this creature's controller controls.",
    ] {
        let d = creature("Sparker", 1, 4, text); admitted(&d).unwrap();
        let mut g = Game::new(vec![d, creature("Bear", 2, 4, "")], 2, 20);
        let source = g.put(0, 0, Zone::Battlefield);
        let mine = g.put(0, 1, Zone::Battlefield);
        let theirs = g.put(1, 1, Zone::Battlefield);
        g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
        pass(&mut g);
        assert_eq!(to(&g, source), 0);
        cite!(["109.5"]; case: text; assert_eq!((to(&g, mine), to(&g, theirs)), if text.contains("opponent") {(0,1)} else {(1,0)}));
    }
}
