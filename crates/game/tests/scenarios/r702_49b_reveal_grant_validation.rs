//! The live reveal grant, as a player's snapshot reports
//! it, through real effects. 602.2a, 701.20a: a card revealed because an ability is activated
//! from a hidden zone stays revealed until the ability leaves the stack. 701.20d: a revealed
//! card put into a library becomes a new object and stops being revealed. 702.49b: the ninjutsu
//! card stays revealed until its ability leaves the stack.
use crate::support::cite;
use game::snapshot::Snapshot;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const KAITO: usize = 0;
const BEAR: usize = 1;
const STASH: usize = 2;

fn setup() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let kaito = CardDef::from_face(faces.iter().find(|f| f.face_name == "Kaito, Bane of Nightmares").unwrap()).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Ninja", "", Some((2, 2))).unwrap();
    let stash = CardDef::new("Stash", "{0}", "Instant", "Put a card from your hand on top of your library.", None).unwrap();
    for d in [&kaito, &bear, &stash] { game::play::check(d).unwrap_or_else(|e| panic!("{}: {e:?}", d.name())); }
    let mut g = Game::new(vec![kaito, bear, stash], 3, 20);
    for p in 0..3 { for _ in 0..15 { g.put(p, BEAR, Zone::Library); } }
    g
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.decision_maker(g.priority), Move::Pass).unwrap(); } }
fn attack(g: &mut Game, a: ObjId) {
    pass(g); pass(g);
    g.act(0, Move::Attack(vec![(a, Target::Player(1))])).unwrap();
    pass(g);
    while let Some(Decision::Blockers { player }) = g.decision { g.act(player, Move::Block(vec![])).unwrap(); }
}
fn mana(g: &mut Game) { g.players[0].pool.colors = [0, 1, 1, 0, 0]; g.players[0].pool.colorless = 1; }
fn ninja(k: ObjId, a: ObjId) -> Move { crate::support::paid("CUB",Move::Activate { source: k, ability: 5, targets: vec![], paying: vec![vec![a]] }) }
/// What `viewer` is shown of these objects, and which hidden cards it is told are revealed.
fn shown(g: &Game, viewer: usize, of: &[ObjId]) -> (Vec<ObjId>, Vec<ObjId>) {
    let seen = Snapshot::of(g, Some(viewer));
    (of.iter().copied().filter(|o| seen.objects.iter().any(|s| s.id == *o)).collect(), seen.publicly_revealed)
}

#[test]
fn r702_49b_the_grant_ends_as_the_ability_resolves_and_the_returned_creature_stays_hidden() {
    let mut g = setup();
    let a = g.put(0, BEAR, Zone::Battlefield);
    let k = g.put(0, KAITO, Zone::Hand);
    let other = g.put(0, BEAR, Zone::Hand);
    attack(&mut g, a);
    mana(&mut g);
    g.act(0, ninja(k, a)).unwrap();
    // the creature returned as a cost is a new object in its owner's hand (400.7): hidden
    let returned = *g.players[0].hand.iter().find(|o| **o != k && **o != other).expect("returned to hand");
    for viewer in [1, 2] {
        cite!(["702.49b", "400.7"]; case: format!("viewer {viewer}"); assert_eq!(shown(&g, viewer, &[k, other, returned]), (vec![k], vec![k]), "viewer {viewer}: only the revealed card"));
    }
    pass(&mut g);
    assert!(g.stack.is_empty());
    let kaito = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(KAITO)).unwrap();
    for viewer in [1, 2] {
        cite!(["702.49b"]; case: format!("viewer {viewer}"); assert_eq!(shown(&g, viewer, &[k, other, returned, kaito]), (vec![kaito], vec![]), "viewer {viewer}: the grant is over"));
    }
}

#[test]
fn r701_20d_a_revealed_card_put_into_its_library_in_response_is_a_new_hidden_object() {
    let mut g = setup();
    let a = g.put(0, BEAR, Zone::Battlefield);
    let k = g.put(0, KAITO, Zone::Hand);
    let other = g.put(0, BEAR, Zone::Hand);
    attack(&mut g, a);
    mana(&mut g);
    g.act(0, ninja(k, a)).unwrap();
    let ability = g.stack[0];
    // in response, its controller puts Kaito on top of their library
    let stash = g.put(0, STASH, Zone::Hand);
    g.act(0, Move::Cast { card: stash, targets: vec![] }).unwrap();
    pass(&mut g);
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    assert!(among.contains(&k));
    g.act(0, Move::Choose(Answer::Objects(vec![k]))).unwrap();
    assert_eq!(g.stack, vec![ability], "the ninjutsu ability is still on the stack");
    let top = *g.players[0].library.last().unwrap();
    assert_eq!(g.obj(top).card, Some(KAITO));
    cite!(["400.7"]; assert_ne!(top, k, "a new object (400.7)"));
    for viewer in [1, 2] {
        cite!(["400.7"]; case: format!("viewer {viewer}"); assert_eq!(shown(&g, viewer, &[k, top, other]), (vec![], vec![]), "viewer {viewer}: no longer revealed, while the ability is still on the stack"));
    }
    // the ability resolves with its card gone from the hand: nothing enters
    pass(&mut g);
    cite!(["400.7"]; assert!(!g.battlefield.iter().any(|o| g.obj(*o).card == Some(KAITO))));
    assert_eq!(shown(&g, 1, &[top]), (vec![], vec![]));
}

#[test]
fn r602_2a_the_grant_is_reported_while_the_activation_is_still_being_paid_for() {
    let mut g = setup();
    let d = CardDef::new("Mana choice", "{0}", "Artifact", "{T}: Scry 1. Add {C}{U}{B}.", None).unwrap();
    game::play::check(&d).unwrap();
    std::sync::Arc::make_mut(&mut g.defs).push(d);
    let source = g.put(0, 3, Zone::Battlefield);
    let a = g.put(0, BEAR, Zone::Battlefield);
    let k = g.put(0, KAITO, Zone::Hand);
    attack(&mut g, a);
    let plan = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source, ability: 0, paying: vec![], adding: vec![] }];
    g.act(0, Move::WithMana(plan, Box::new(ninja(k, a)))).unwrap();
    assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::Arrange { .. }, .. })), "the mana ability pauses the payment");
    for branch in [g.clone(), g] {
        cite!(["602.2a"]; assert_eq!(shown(&branch, 1, &[k]), (vec![k], vec![k]), "announced: revealed from 602.2a on, in a copy too"));
    }
}

/// Two Kaitos revealed by pending ninjutsu abilities (702.49b): a player's prompt names them by
/// where they are in the hand, whichever was made first.
fn revealed_labels(made_backwards: bool) -> Vec<u64> {
    let mut g = setup();
    let attackers = [g.put(0, BEAR, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield)];
    let mut kaitos = [g.put(0, KAITO, Zone::Hand), g.put(0, KAITO, Zone::Hand)];
    if made_backwards { g.players[0].hand.reverse(); kaitos.reverse(); }
    pass(&mut g); pass(&mut g);
    g.act(0, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap();
    pass(&mut g);
    while let Some(Decision::Blockers { player }) = g.decision { g.act(player, Move::Block(vec![])).unwrap(); }
    for (k, a) in kaitos.iter().zip(attackers) {
        g.players[0].pool.colors = [0, 1, 1, 0, 0]; g.players[0].pool.colorless = 1;
        g.act(0, ninja(*k, a)).unwrap();
    }
    cite!(["702.49b"]; assert_eq!(shown(&g, 1, &kaitos).1.len(), 2, "both are revealed"));
    let mut s = game::simulation::Simulation::new(g);
    s.prompt(1).unwrap();
    let labels = s.labels(1).unwrap();
    kaitos.iter().map(|k| labels.number(*k).unwrap()).collect()
}

#[test]
fn r702_49b_revealed_hand_cards_are_labelled_by_their_place_in_the_hand_not_by_allocation() {
    // kaitos[0] is the first in the hand in both games
    assert_eq!(revealed_labels(false), revealed_labels(true));
}
