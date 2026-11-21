//! Bolster (701.39a) and support (701.41a): the least toughness is read as bolster
//! resolves, after a pump or a death in response; bolster with no creature does nothing;
//! Dromoka Captain's attack trigger; support's "up to" targets: none, one gone by
//! resolution, the same creature twice.
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const SCAVENGER: usize = 0;
const RAPTOR: usize = 1;
const LEAD: usize = 2;
const CAPTAIN: usize = 3;
const GROWTH: usize = 4;
const MURDER: usize = 5;
const BEAR: usize = 6;
const MITE: usize = 7;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let real = |name: &str| CardDef::from_faces(&catalog[name]).unwrap();
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    let defs = vec![real("Sandsteppe Scavenger"), real("Expedition Raptor"), real("Lead by Example"), real("Dromoka Captain"),
        c("Growth", "Instant", "Target creature gets +0/+3 until end of turn.", None),
        c("Murder", "Instant", "Destroy target creature.", None),
        c("Bear", "Creature — Bear", "", Some((2, 2))),
        c("Mite", "Creature — Insect", "", Some((0, 1)))];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, BEAR, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn mana(g: &mut Game) { for p in 0..2 { g.players[p].pool.colors = [9; 5]; g.players[p].pool.colorless = 9; } }
fn cast(g: &mut Game, player: usize, card: usize, targets: Vec<Vec<Target>>) -> Result<(), String> {
    let spell = g.put(player, card, Zone::Hand);
    mana(g);
    g.act(player, crate::support::paid(["CCCCG", "CCCWW", "CG", "CCW", "", "", "", ""][card], Move::Cast { card: spell, targets }))
}
/// Pass until the stack is empty or a decision is asked.
fn settle(g: &mut Game) { while !g.stack.is_empty() && g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); } }
/// Pass until the Scavenger is on the battlefield, its trigger on the stack.
fn entered(g: &mut Game) -> ObjId {
    while !g.battlefield.iter().any(|o| g.obj(*o).chars.name == "Sandsteppe Scavenger") { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(g.stack.len(), 1, "its trigger");
    while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    named(g, "Sandsteppe Scavenger")
}
fn plus(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("+1/+1").copied().unwrap_or(0) }
fn named(g: &Game, name: &str) -> ObjId { *g.battlefield.iter().find(|o| g.obj(**o).chars.name == name).unwrap() }

/// Bear and Scavenger tie at 2 as the trigger goes on the stack; the Bear gets +0/+3 in
/// response, so only the Scavenger has the least toughness as bolster resolves: no choice.
#[test]
fn bolster_reads_toughness_as_it_resolves() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    cast(&mut g, 0, SCAVENGER, vec![]).unwrap();
    let scavenger = entered(&mut g);
    cast(&mut g, 0, GROWTH, vec![vec![Target::Object(bear)]]).unwrap();
    settle(&mut g);
    cite!(["701.39a", "608.2h"]; assert!(g.decision.is_none(), "no tie left: {:?}", g.decision));
    cite!(["701.39a"]; assert_eq!((plus(&g, scavenger), plus(&g, bear)), (2, 0)));
}

/// The Scavenger dies in response: the Bear is the only creature, and gets the counters; with
/// no creature at all, nothing happens.
#[test]
fn bolster_after_a_death_in_response() {
    for with_bear in [true, false] {
        let mut g = game();
        let bear = with_bear.then(|| g.put(0, BEAR, Zone::Battlefield));
        cast(&mut g, 0, SCAVENGER, vec![]).unwrap();
        let scavenger = entered(&mut g);
        cast(&mut g, 0, MURDER, vec![vec![Target::Object(scavenger)]]).unwrap();
        settle(&mut g);
        cite!(["609.3"]; case: format!("with_bear={with_bear}"); assert!(g.decision.is_none() && g.stack.is_empty(), "{:?}", g.decision));
        if let Some(bear) = bear { cite!(["701.39a"]; assert_eq!(plus(&g, bear), 2)); }
    }
}

/// Dromoka Captain (1/1) attacks beside a 0/1 Mite and a Bear: the Mite and the Captain tie at
/// toughness 1, each a choice; the Bear is not.
#[test]
fn dromoka_captains_attack_bolsters_one_of_the_tied() {
    let mut g = game();
    let captain = g.put(0, CAPTAIN, Zone::Battlefield);
    let mite = g.put(0, MITE, Zone::Battlefield);
    g.put(0, BEAR, Zone::Battlefield);
    // to player 0's next turn, when the Captain may attack
    let step = |g: &mut Game| match g.decision {
        Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
        Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
        _ => g.act(g.priority, Move::Pass).unwrap(),
    };
    while g.active == 0 { step(&mut g); }
    while g.decision != Some(Decision::Attackers { player: 0 }) { step(&mut g); }
    g.act(0, Move::Attack(vec![(captain, Target::Player(1))])).unwrap();
    settle(&mut g);
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, count: 1, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    let mut among = among; among.sort();
    let mut tied = vec![captain, mite]; tied.sort();
    cite!(["701.39a"]; assert_eq!(among, tied));
    g.act(0, Move::Choose(Answer::Objects(vec![mite]))).unwrap();
    settle(&mut g);
    cite!(["701.39a"]; assert_eq!((plus(&g, mite), plus(&g, captain)), (1, 0)));
}

/// Lead by Example: "up to two" — no target at all is a legal cast; the same creature twice is
/// not (601.2c); one of two gone by resolution, the other still gets its counter (608.2b).
#[test]
fn support_up_to_n_targets() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let theirs = g.put(1, BEAR, Zone::Battlefield);
    let mut none = g.clone();
    cast(&mut none, 0, LEAD, vec![vec![]]).unwrap();
    settle(&mut none);
    cite!(["701.41a"]; assert!(none.stack.is_empty()));
    cite!(["115.3"]; refused("the same target twice for one \"target\" (115.3)"): cast(&mut g.clone(), 0, LEAD, vec![vec![Target::Object(bear), Target::Object(bear)]]));
    cast(&mut g, 0, LEAD, vec![vec![Target::Object(bear), Target::Object(theirs)]]).unwrap();
    g.act(0, Move::Pass).unwrap();
    cast(&mut g, 1, MURDER, vec![vec![Target::Object(theirs)]]).unwrap();
    settle(&mut g);
    assert!(!g.battlefield.contains(&theirs));
    cite!(["608.2b"]; assert_eq!(plus(&g, bear), 1));
}

/// Expedition Raptor's trigger with no other creature: no target is needed and it does
/// nothing (up to two).
#[test]
fn support_with_no_other_creature() {
    let mut g = game();
    cast(&mut g, 0, RAPTOR, vec![]).unwrap();
    for _ in 0..10 {
        match g.decision.clone() {
            Some(Decision::Targets { player, .. }) => { g.act(player, Move::ChooseTargets(vec![vec![]])).unwrap(); }
            None if g.stack.is_empty() => break,
            None => g.act(g.priority, Move::Pass).unwrap(),
            Some(d) => panic!("{d:?}"),
        }
    }
    assert!(g.stack.is_empty());
    cite!(["701.41a"]; assert_eq!(plus(&g, named(&g, "Expedition Raptor")), 0));
}
