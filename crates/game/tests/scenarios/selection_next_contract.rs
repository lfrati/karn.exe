//! Target declaration versus resolution selection: CR 115.1, 601.2c, 603.3d,
//! 608.2b–d and 400.7. Refusals below are engine limitations, not Magic outcomes.
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, Zone};

fn native(names: &[&str]) -> Vec<CardDef> {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    names.iter().map(|name| CardDef::from_faces(&catalog[name]).unwrap()).collect()
}
fn pass_round(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

#[test]
fn r603_native_target_announcement_restricts_controller_and_binds_that_creature() {

    let defs = native(&["Floodpits Drowner", "Llanowar Elves", "Island"]);
    for d in &defs {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(defs, 2, 20);
    let own = g.put(0, 1, Zone::Battlefield);
    let first = g.put(1, 1, Zone::Battlefield);
    let second = g.put(1, 1, Zone::Battlefield);
    let drowner = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[1] = 3;
    g.act(0, paid("UU",Move::Cast { card: drowner, targets: vec![] })).unwrap();
    pass_round(&mut g);
    cite!(["115.1d", "603.3d"]; assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. }))));
    let before = format!("{g:?}");
    cite!(["603.3d"]; refused("an illegal target, Llanowar Elves"): g.act(0, Move::ChooseTargets(vec![vec![own.into()]])));
    assert_eq!(format!("{g:?}"), before);
    cite!(["603.3d"]; refused("that decision is another player's"): g.act(1, Move::ChooseTargets(vec![vec![first.into()]])));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::ChooseTargets(vec![vec![first.into()]])).unwrap();
    cite!(["603.3d"]; assert!(!g.obj(first).tapped && !g.obj(second).tapped));
    pass_round(&mut g);
    cite!(["608.2c"]; assert!(g.obj(first).tapped && !g.obj(second).tapped && !g.obj(own).tapped));
    cite!(["122.6"]; assert_eq!(g.obj(first).counters.get("stun"), Some(&1)));
    assert!(!g.obj(second).counters.contains_key("stun") && !g.obj(own).counters.contains_key("stun"));
    cite!(["608.2d"]; assert!(g.decision.is_none() && g.stack.is_empty(), "announced target is not chosen again"));
}

#[test]
fn r608_native_spell_target_is_fixed_and_a_zone_change_makes_it_illegal() {
    let defs = native(&["Unsummon", "Llanowar Elves"]);
    for d in &defs {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(defs, 2, 20);
    let bear = g.put(1, 1, Zone::Battlefield);
    let other = g.put(1, 1, Zone::Battlefield);
    let first = g.put(0, 0, Zone::Hand);
    let response = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[1] = 2;
    g.act(0, paid("U", Move::Cast { card: first, targets: vec![vec![bear.into()]] })).unwrap();
    cite!(["601.2c"]; assert!(g.decision.is_none() && g.battlefield.contains(&bear)));
    g.act(0, paid("U", Move::Cast { card: response, targets: vec![vec![bear.into()]] })).unwrap();
    pass_round(&mut g);
    cite!(["400.7"]; assert!(!g.battlefield.contains(&bear) && !g.players[1].hand.contains(&bear)));
    cite!(["608.2c"]; assert_eq!(g.players[1].hand.len(), 1));
    pass_round(&mut g);
    cite!(["608.2b"]; assert!(g.stack.is_empty() && g.decision.is_none()));
    cite!(["608.2b"]; assert!(g.battlefield.contains(&other), "no substitution with another creature"));
    assert_eq!(g.players[0].graveyard.len(), 2);
    assert_eq!(g.players[1].hand.len(), 1);
}

#[test]
fn r608_native_resolution_selection_suspends_and_retains_the_offered_group() {

    let mut g = Game::new(native(&["Sleight of Hand", "Island", "Forest"]), 2, 20);
    let bottom = g.put(0, 2, Zone::Library);
    let top = g.put(0, 1, Zone::Library);
    let spell = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[1] = 1;
    g.act(0, paid("U",Move::Cast { card: spell, targets: vec![] })).unwrap();
    assert!(g.decision.is_none());
    pass_round(&mut g);
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, count: 1, .. } }) = &g.decision else {
        panic!("{:?}", g.decision);
    };
    cite!(["608.2d"]; assert_eq!(among, &vec![top, bottom]));
    cite!(["701.20e"]; assert!(g.observations(1).is_empty()));
    let before = format!("{g:?}");
    cite!(["608.2d"]; refused("that decision is another player's"): g.act(1, Move::Choose(Answer::Objects(vec![top]))));
    assert_eq!(format!("{g:?}"), before);
    cite!(["608.2d"]; refused("a decision is pending: a choice of the effect being applied"): g.act(0, Move::Pass));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::Choose(Answer::Objects(vec![top]))).unwrap();
    cite!(["608.2c"]; assert!(g.stack.is_empty() && g.decision.is_none()));
    assert_eq!(g.players[0].hand.len(), 1);
    assert_eq!(g.obj(g.players[0].hand[0]).chars.name, "Island");
    assert_eq!(g.players[0].library.len(), 1);
    assert_eq!(g.obj(g.players[0].library[0]).chars.name, "Forest");
    cite!(["608.2c"]; assert_eq!(g.observations(0).len(), 1, "resume does not repeat the look"));
}
