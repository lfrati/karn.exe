//! Outlaw is the union of five effective subtypes, in the text's own domain (700.12).
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use std::sync::OnceLock;
fn native(name: &str) -> CardDef {
    static FACES: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    let faces = FACES.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    CardDef::from_faces(&CardDef::catalog(faces)[name]).unwrap_or_else(|e| panic!("{name}: {e}"))
}
fn card(name: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    CardDef::new(name, "{0}", types, text, pt).unwrap()
}
fn game(defs: Vec<CardDef>) -> Game {
    for d in &defs {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {e:?}", d.name()));
    }
    Game::new(defs, 2, 20)
}
fn pass(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn settle(g: &mut Game) {
    for _ in 0..20 {
        if g.stack.is_empty() {
            return;
        }
        pass(g);
    }
    panic!("unsettled");
}
fn cast(g: &mut Game, d: usize, targets: Vec<Vec<Target>>, spending: &str) -> ObjId {
    let c = g.put(0, d, Zone::Hand);
    g.act(0, paid(spending,Move::Cast { card: c, targets })).unwrap();
    c
}
fn count() -> CardDef {
    card("Count", "Sorcery", "You gain life equal to the number of outlaws you control.", None)
}
fn bear() -> CardDef {
    card("Bear", "Creature — Bear", "", Some((4, 4)))
}
fn frog() -> CardDef {
    card("Frog", "Instant", "Target creature becomes a Frog creature until end of turn.", None)
}

#[test]
fn r700_12_each_arm_and_overlapping_types_count_once() {
    for ty in ["Assassin", "Mercenary", "Pirate", "Rogue", "Warlock", "Assassin Mercenary Pirate Rogue Warlock"] {
        let donor = card("Outlaw", &format!("Creature — {ty}"), "", Some((2, 2)));
        let mut g = game(vec![count(), donor, bear()]);
        g.put(0, 1, Zone::Battlefield);
        g.put(0, 2, Zone::Battlefield);
        g.put(1, 1, Zone::Battlefield);
        cast(&mut g, 0, vec![], "");
        settle(&mut g);
        cite!(["700.12","700.12a"];assert_eq!(g.players[0].life,21));
    }
}
#[test]
fn r700_12_negation_is_complement_and_empty_is_zero() {
    let not =
        card("Nonoutlaws", "Sorcery", "You gain life equal to the number of non-outlaw creatures you control.", None);
    let outlaw = card("Overlap", "Creature — Rogue Pirate", "", Some((2, 2)));
    let mut g = game(vec![count(), not, outlaw, bear()]);
    cast(&mut g, 0, vec![], "");
    settle(&mut g);
    assert_eq!(g.players[0].life, 20);
    g.put(0, 2, Zone::Battlefield);
    g.put(0, 3, Zone::Battlefield);
    cast(&mut g, 1, vec![], "");
    settle(&mut g);
    cite!(["700.12"];assert_eq!(g.players[0].life,21));
}
#[test]
fn r700_12a_native_noncreature_changeling_counts_only_on_battlefield() {
    let mut g = game(vec![count(), native("Firdoch Core")]);
    let c = g.put(0, 1, Zone::Hand);
    cast(&mut g, 0, vec![], "");
    settle(&mut g);
    assert_eq!(g.players[0].life, 20);
    g.move_object(c, Zone::Battlefield);
    cast(&mut g, 0, vec![], "");
    settle(&mut g);
    cite!(["700.12a","702.73a"];assert_eq!(g.players[0].life,21));
}
#[test]
fn r700_12_current_layer_type_loss_removes_membership() {
    let rogue = card("Rogue", "Creature — Rogue", "", Some((4, 4)));
    let mut g = game(vec![count(), frog(), rogue]);
    let r = g.put(0, 2, Zone::Battlefield);
    cast(&mut g, 0, vec![], "");
    settle(&mut g);
    assert_eq!(g.players[0].life, 21);
    g.players[0].pool.colors[ir::Color::Black as usize] = 2;
    cast(&mut g, 1, vec![vec![Target::Object(r)]], "");
    settle(&mut g);
    assert_eq!(g.power(r), Some(4));
    cast(&mut g, 0, vec![], "");
    settle(&mut g);
    cite!(["700.12","613.1d","702.73a"];assert_eq!(g.players[0].life,21));
}
#[test]
fn r700_12_native_kindred_artifact_spell_copied_once_not_recursively_cast() {
    let mut g = game(vec![native("Double Down"), native("Firdoch Core"), count()]);
    g.put(0, 0, Zone::Battlefield);
    g.players[0].pool.colors[ir::Color::Blue as usize] = 3;
    cast(&mut g, 1, vec![], "UUU");
    cite!(["700.12","702.73a","603.2c"];assert_eq!(g.stack.len(),2));
    pass(&mut g);
    assert_eq!(g.stack.len(), 2);
    settle(&mut g);
    cast(&mut g, 2, vec![], "");
    settle(&mut g);
    cite!(["707.10","608.3f"];assert_eq!(g.players[0].life,22));
}
#[test]
fn r700_12_native_kindred_instant_spell_is_outlaw_without_being_creature() {
    let mut g = game(vec![native("Double Down"), native("Crib Swap"), bear()]);
    g.put(0, 0, Zone::Battlefield);
    let b = g.put(0, 2, Zone::Battlefield);
    g.players[0].pool.colors[ir::Color::White as usize] = 3;
    cast(&mut g, 1, vec![vec![Target::Object(b)]], "WWW");
    cite!(["700.12","702.73a"];assert_eq!(g.stack.len(),2));
    pass(&mut g);
    assert_eq!(g.stack.len(), 2);
    pass(&mut g);
    assert!(!g.battlefield.contains(&b));
    settle(&mut g);
    cite!(["707.10","608.2b"];assert_eq!(g.battlefield.len(),2));
}
#[test]
fn r700_12_native_mine_raider_rechecks_another_outlaw() {
    let mut g = game(vec![native("Mine Raider"), native("Firdoch Core")]);
    let core = g.put(0, 1, Zone::Battlefield);
    g.players[0].pool.colors[ir::Color::Red as usize] = 3;
    cast(&mut g, 0, vec![], "RRR");
    pass(&mut g);
    assert_eq!(g.stack.len(), 1);
    g.move_object(core, Zone::Graveyard);
    settle(&mut g);
    cite!(["603.4","700.12a"];assert_eq!(g.battlefield.len(),1));
}
#[test]
fn r700_12_native_rakish_crew_uses_death_lki_once_with_source_departure() {
    let rogue = card("Overlap", "Creature — Rogue Pirate", "", Some((2, 2)));
    let destroy = card("Sweep", "Sorcery", "Destroy all nonland permanents.", None);
    let mut g = game(vec![native("Rakish Crew"), rogue, destroy]);
    let crew = g.put(0, 0, Zone::Battlefield);
    let rogue = g.put(0, 1, Zone::Battlefield);
    g.put(1, 1, Zone::Battlefield);
    cast(&mut g, 2, vec![], "");
    pass(&mut g);
    // CR404.3: explicitly authored owner order, separate from selection.
    assert_eq!(g.decision, Some(game::state::Decision::Choose { player: 0, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: vec![crew,rogue] } }));
    g.act(0, Move::Choose(game::state::Answer::Objects(vec![rogue,crew]))).unwrap();

    cite!(["603.10a","700.12","603.2c"];assert_eq!(g.stack.len(),1));
    settle(&mut g);
    assert_eq!((g.players[0].life, g.players[1].life), (21, 19));
}
#[test]
fn r700_12_newly_exposed_unrelated_full_card_clauses_still_refuse() {
    // These are repository limits, not restrictions imposed by the Magic rules.
    for (name, reason) in [
        ("Take the Fall", "modifier: Instead of Gets"),
        ("Nameless Inversion", "loses: types, other abilities or this ability"),
    ] {
        let def = native(name);
        let error = game::play::check(&def).unwrap_err();
        assert!(error.0.contains(reason), "{name}: {error:?}");
    }
}
#[test]
fn r700_12_native_nonoutlaw_target_rejects_otherwise_legal_outlaw() {
    let outlaw = card("Rogue", "Creature — Rogue", "", Some((2, 2)));
    let mut g = game(vec![native("Shoot the Sheriff"), outlaw, bear()]);
    let r = g.put(1, 1, Zone::Battlefield);
    let b = g.put(1, 2, Zone::Battlefield);
    g.players[0].pool.colors[ir::Color::Black as usize] = 2;
    let c = g.put(0, 0, Zone::Hand);
    let before=format!("{g:?}");
    cite!(["700.12","601.2c"]; refused("an illegal target, Rogue"): g.act(0,paid("BB",Move::Cast{card:c,targets:vec![vec![Target::Object(r)]]})));
    assert_eq!(format!("{g:?}"),before);
    g.act(0, paid("BB",Move::Cast { card: c, targets: vec![vec![Target::Object(b)]] })).unwrap();
    settle(&mut g);
    assert!(!g.battlefield.contains(&b));
    assert!(g.battlefield.contains(&r));
}

#[test]
fn r700_12_death_does_not_use_reappearing_printed_types_in_graveyard() {
    let rogue = card("Rogue", "Creature — Rogue", "", Some((4, 4)));
    let kill = card("Kill", "Instant", "Destroy target creature.", None);
    let mut g = game(vec![native("Rakish Crew"), frog(), rogue, kill]);
    g.put(0, 0, Zone::Battlefield);
    let r = g.put(0, 2, Zone::Battlefield);
    g.players[0].pool.colors[ir::Color::Black as usize] = 2;
    cast(&mut g, 1, vec![vec![Target::Object(r)]], "");
    settle(&mut g);
    cast(&mut g, 3, vec![vec![Target::Object(r)]], "");
    pass(&mut g);
    cite!(["700.12","603.10a","400.7"];assert!(g.stack.is_empty()));
    assert_eq!((g.players[0].life, g.players[1].life), (20, 20));
}
#[test]
fn r700_12_native_other_and_controller_static_scopes_survive_union() {
    let rogue = card("Rogue", "Creature — Rogue", "", Some((2, 2)));
    let mut g = game(vec![native("Hellspur Posse Boss"), rogue, bear()]);
    let boss = g.put(0, 0, Zone::Battlefield);
    let own = g.put(0, 1, Zone::Battlefield);
    let enemy = g.put(1, 1, Zone::Battlefield);
    let bear = g.put(0, 2, Zone::Battlefield);
    cite!(["700.12a","109.5"];assert!(g.seen(own,None).has(ir::Keyword::from_name("Haste").unwrap())));
    assert!(!g.seen(boss, None).has(ir::Keyword::from_name("Haste").unwrap()));
    assert!(!g.seen(enemy, None).has(ir::Keyword::from_name("Haste").unwrap()));
    assert!(!g.seen(bear, None).has(ir::Keyword::from_name("Haste").unwrap()));
}
#[test]
fn r700_12_native_turn_and_controller_first_strike_scopes() {
    let rogue = card("Rogue", "Creature — Rogue", "", Some((2, 2)));
    let mut g = game(vec![native("At Knifepoint"), rogue]);
    g.put(0, 0, Zone::Battlefield);
    let own = g.put(0, 1, Zone::Battlefield);
    let enemy = g.put(1, 1, Zone::Battlefield);
    assert!(g.seen(own, None).has(ir::Keyword::from_name("First strike").unwrap()));
    assert!(!g.seen(enemy, None).has(ir::Keyword::from_name("First strike").unwrap()));
    g.active = 1;
    cite!(["700.12a","109.5"];assert!(!g.seen(own,None).has(ir::Keyword::from_name("First strike").unwrap())));
}
#[test]
fn r700_12_native_noncreature_outlaw_enter_triggers_once() {
    let mut g = game(vec![native("Vial Smasher, Gleeful Grenadier"), native("Firdoch Core")]);
    g.put(0, 0, Zone::Battlefield);
    g.players[0].pool.colors[ir::Color::Blue as usize] = 3;
    cast(&mut g, 1, vec![], "UUU");
    pass(&mut g);
    // Target selection puts this genuine triggered occurrence onto the stack.
    g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    cite!(["700.12","702.73a","603.2c"];assert_eq!(g.stack.len(),1));
    settle(&mut g);
    assert_eq!(g.players[1].life, 19);
}
