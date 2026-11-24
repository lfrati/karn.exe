//! "this creature gets +1/+0 until end of turn, then scry 1", "it deals damage equal to its
//! power to its controller, then destroy that creature": after a clause said of an object,
//! ", then" sets off an instruction the ability's controller follows next (608.2c). Every card
//! is read from its Oracle text.
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const PROPHET: usize = 0;
const FEROCITY: usize = 1;
const SHOCK: usize = 2;
const GIANT: usize = 3;
const MOUNTAIN: usize = 4;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs: Vec<_> = ["Burning Prophet", "Consuming Ferocity", "Shock", "Hill Giant", "Mountain"]
        .map(|n| CardDef::from_faces(&catalog[n]).unwrap_or_else(|e| panic!("{n}: {e:?}"))).to_vec();
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..12 { g.put(p, MOUNTAIN, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

fn mana(g: &mut Game) { g.players[0].pool.colors = [0, 0, 0, 5, 0]; g.players[0].pool.colorless = 5; }

/// Passes priority until the stack is empty or a question other than priority is asked.
fn settle(g: &mut Game) {
    for _ in 0..30 {
        if g.decision.is_some() || g.stack.is_empty() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("the stack did not settle");
}

/// To player 0's next upkeep, with its triggered abilities on the stack.
fn upkeep(g: &mut Game) {
    let turn = g.turn;
    for _ in 0..200 {
        if g.turn > turn && g.active == 0 && g.part == Part::Upkeep && !g.stack.is_empty() { return; }
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            None => g.act(g.priority, Move::Pass).unwrap(),
            other => panic!("unexpected question {other:?}"),
        }
    }
    panic!("no upkeep trigger");
}

#[test]
fn r608_2c_burning_prophet_gets_bigger_then_its_controller_scries() {
    let mut g = game();
    let prophet = g.put(0, PROPHET, Zone::Battlefield);
    let top = *g.players[0].library.last().unwrap();
    mana(&mut g);
    let shock = g.put(0, SHOCK, Zone::Hand);
    g.act(0, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Player(1)]] })).unwrap();
    cite!(["603.2"]; assert_eq!(g.stack.len(), 2, "the trigger, above Shock"));
    settle(&mut g);
    cite!(["608.2c", "701.22a"]; assert!(matches!(&g.decision, Some(Decision::Choose { player: 0, choice: Choice::Arrange { cards, .. } }) if cards == &vec![top]), "{:?}", g.decision));
    cite!(["608.2c", "611.2a"]; assert_eq!(g.power(prophet), Some(2), "it got +1/+0 first"));
    cite!(["405.5"]; assert_eq!(g.players[1].life, 20, "Shock has not resolved yet"));
    g.act(0, Move::Choose(Answer::Arrange { top: vec![], away: vec![top] })).unwrap();
    settle(&mut g);
    cite!(["701.22a"]; assert_eq!(g.players[0].library[0], top, "scried to the bottom"));
    cite!(["120.3a"]; assert_eq!(g.players[1].life, 18));
    cite!(["611.2a"]; assert_eq!(g.power(prophet), Some(2)));
}

#[test]
fn r608_2c_consuming_ferocity_deals_damage_then_destroys_the_creature() {
    let mut g = game();
    let giant: ObjId = g.put(1, GIANT, Zone::Battlefield);
    mana(&mut g);
    let aura = g.put(0, FEROCITY, Zone::Hand);
    g.act(0, paid("CR", Move::Cast { card: aura, targets: vec![vec![Target::Object(giant)]] })).unwrap();
    settle(&mut g);
    cite!(["303.4a"]; assert_eq!(g.power(giant), Some(4)));
    for counters in [1, 2] {
        upkeep(&mut g);
        settle(&mut g);
        cite!(["122.1a"]; case: format!("{counters}"); assert_eq!(g.power(giant), Some(4 + counters)));
        cite!(["608.2c"]; case: format!("{counters}"); assert!(g.battlefield.contains(&giant) && g.players[1].life == 20, "fewer than three counters"));
    }
    upkeep(&mut g);
    settle(&mut g);
    cite!(["608.2c", "120.3a"]; assert_eq!(g.players[1].life, 13, "its power, 3 + 1 + 3, to its controller"));
    cite!(["608.2c", "701.8a"]; assert!(!g.battlefield.contains(&giant), "then it is destroyed"));
    cite!(["704.5m"]; assert!(g.players[0].graveyard.iter().any(|c| g.obj(*c).card == Some(FEROCITY))));
}
