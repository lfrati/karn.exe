//! Valiant, from Heartfire Hero's rulings: a spell whose target is changed to
//! the creature makes it a target then (115.7); a copy given it as a new target does too
//! (707.10c), counted by the copy's controller; a creature that left and came back is a new
//! object never yet targeted (400.7).
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const HERO: usize = 0;
const PUMP: usize = 1;
const BEAR: usize = 2;
const REDIRECT: usize = 3;
const TWIN: usize = 4;
const FLICKER: usize = 5;
const ISLAND: usize = 6;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_faces(&CardDef::catalog(&faces)[name]).unwrap();
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    let defs = vec![real("Heartfire Hero"),
        c("Pump", "Instant", "Target creature gets +0/+0 until end of turn.", None),
        c("Bear", "Creature — Bear", "", Some((2, 2))),
        real("Redirect"),
        c("Twin", "Instant", "Copy target spell. You may choose new targets for the copy.", None),
        c("Flicker", "Instant", "Exile target creature, then return it to the battlefield under its owner's control.", None),
        real("Island")];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    for p in 0..2 { g.players[p].pool.colors = [20; 5]; g.players[p].pool.colorless = 20; }
    g
}
/// Resolve everything; any new target asked for is `to`, any "may" is yes.
fn resolve(g: &mut Game, to: Option<ObjId>) {
    for _ in 0..60 {
        match g.decision.clone() {
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
            Some(Decision::Choose { player, choice: Choice::May }) => g.act(player, Move::Choose(Answer::Yes)).unwrap(),
            Some(Decision::Choose { player, choice: Choice::ChangeTargets { .. } | Choice::Retarget { .. } }) =>
                g.act(player, Move::Choose(Answer::Targets(vec![vec![Target::Object(to.expect("a new target"))]]))).unwrap(),
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            Some(d) => panic!("{d:?}"),
        }
    }
    panic!("did not resolve");
}
fn cast(g: &mut Game, player: usize, card: usize, at: ObjId, spending: &str) -> ObjId {
    let spell = g.put(player, card, Zone::Hand);
    while g.priority != player { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(player, paid(spending, Move::Cast { card: spell, targets: vec![vec![Target::Object(at)]] })).unwrap();
    *g.stack.last().unwrap()
}
fn counters(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("+1/+1").copied().unwrap_or(0) }

#[test]
fn a_spell_redirected_to_it_makes_it_a_target_the_first_time() {
    let mut g = game();
    let hero = g.put(0, HERO, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let pump = cast(&mut g, 0, PUMP, bear, "");
    cast(&mut g, 0, REDIRECT, pump, "UU");
    resolve(&mut g, Some(hero));
    cite!(["115.7"]; assert_eq!(counters(&g, hero), 1));
}

/// Player 0's copy of its own Pump, given the Hero as its new target, triggers valiant; an
/// opponent's copy aimed at it does not, nor does it use up the first time.
#[test]
fn a_copy_given_it_as_a_new_target_counts_for_the_copys_controller() {
    for copier in [1, 0] {
        let mut g = game();
        let hero = g.put(0, HERO, Zone::Battlefield);
        let bear = g.put(0, BEAR, Zone::Battlefield);
        let pump = cast(&mut g, 0, PUMP, bear, "");
        cast(&mut g, copier, TWIN, pump, "");
        resolve(&mut g, Some(hero));
        cite!(["707.10c", "707.10"]; case: format!("copier {copier}"); assert_eq!(counters(&g, hero), u32::from(copier == 0), "copied by player {copier}"));
        if copier == 1 {
            cast(&mut g, 0, PUMP, hero, "");
            resolve(&mut g, None);
            cite!(["707.10"]; case: format!("copier {copier}"); assert_eq!(counters(&g, hero), 1, "the first time player 0 targets it"));
        }
    }
}

#[test]
fn a_hero_that_returned_is_targeted_for_the_first_time() {
    let mut g = game();
    let hero = g.put(0, HERO, Zone::Battlefield);
    cast(&mut g, 0, PUMP, hero, "");
    resolve(&mut g, None);
    assert_eq!(counters(&g, hero), 1);
    cast(&mut g, 0, FLICKER, hero, "");
    resolve(&mut g, None);
    let back = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(HERO)).unwrap();
    cite!(["400.7"]; assert_ne!(back, hero));
    cast(&mut g, 0, PUMP, back, "");
    resolve(&mut g, None);
    cite!(["400.7"]; assert_eq!(counters(&g, back), 1));
}
