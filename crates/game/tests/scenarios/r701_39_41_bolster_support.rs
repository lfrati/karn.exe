//! Bolster (701.39a) and support (701.41a) are the instructions their rules say they are:
//! the choice of a creature with the least toughness among creatures you control, as the
//! ability resolves, and up to N other target creatures on a permanent, up to N target
//! creatures on an instant or sorcery.
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, Target, Zone};

const SCAVENGER: usize = 0;
const RAPTOR: usize = 1;
const LEAD: usize = 2;
const BEAR: usize = 3;
const OX: usize = 4;
const MITE: usize = 5;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let defs = vec![
        real("Sandsteppe Scavenger"),
        real("Expedition Raptor"),
        real("Lead by Example"),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Ox", "{0}", "Creature — Ox", "", Some((1, 4))).unwrap(),
        CardDef::new("Mite", "{0}", "Creature — Insect", "", Some((0, 1))).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, BEAR, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn cast(g: &mut Game, card: usize, targets: Vec<Vec<Target>>) {
    let spell = g.put(0, card, Zone::Hand);
    g.players[0].pool.colors = [9; 5];
    g.players[0].pool.colorless = 9;
    g.act(0, crate::support::paid(["CCCCG", "CCCWW", "CG", "", "", ""][card], Move::Cast { card: spell, targets })).unwrap();
}
fn settle(g: &mut Game) {
    while !g.stack.is_empty() && g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn plus(g: &Game, o: game::ObjId) -> u32 { g.obj(o).counters.get("+1/+1").copied().unwrap_or(0) }

#[test]
fn r701_39a_bolster_puts_its_counters_on_the_creature_with_the_least_toughness() {
    let mut g = game();
    let ox = g.put(0, OX, Zone::Battlefield);
    let theirs = g.put(1, MITE, Zone::Battlefield);
    cast(&mut g, SCAVENGER, vec![]);
    settle(&mut g);
    // the Scavenger (2/2) has less toughness than the Ox; an opponent's creature isn't among
    // creatures you control
    let scavenger = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Sandsteppe Scavenger").unwrap();
    cite!(["701.39a"]; assert_eq!((plus(&g, scavenger), plus(&g, ox), plus(&g, theirs)), (2, 0, 0)));
}

#[test]
fn r701_39a_tied_creatures_are_each_a_choice() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.put(0, OX, Zone::Battlefield);
    cast(&mut g, SCAVENGER, vec![]);
    settle(&mut g);
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, count: 1, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    let scavenger = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Sandsteppe Scavenger").unwrap();
    let mut among = among.clone(); among.sort();
    let mut tied = vec![bear, scavenger]; tied.sort();
    cite!(["701.39a"]; assert_eq!(among, tied));
    g.act(0, Move::Choose(Answer::Objects(vec![bear]))).unwrap();
    settle(&mut g);
    cite!(["701.39a"]; assert_eq!((plus(&g, bear), plus(&g, scavenger)), (2, 0)));
}

#[test]
fn r701_41a_support_on_a_permanent_targets_other_creatures_and_on_a_spell_any() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let theirs = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, RAPTOR, vec![]);
    settle(&mut g);
    let raptor = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Expedition Raptor").unwrap();
    assert!(matches!(g.decision, Some(Decision::Targets { player: 0, groups: 1, .. })), "{:?}", g.decision);
    // "other": the Raptor itself isn't a legal target
    cite!(["115.1"]; refused("an illegal target, Expedition Raptor: it is the object whose ability this is, and the target says \"another\" (115.1)"): g.act(0, Move::ChooseTargets(vec![vec![Target::Object(raptor)]])));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bear), Target::Object(theirs)]])).unwrap();
    settle(&mut g);
    cite!(["701.41a"]; assert_eq!((plus(&g, bear), plus(&g, theirs), plus(&g, raptor)), (1, 1, 0)));
    // the instant has no "other": up to two of any creatures, the Raptor among them
    cast(&mut g, LEAD, vec![vec![Target::Object(raptor)]]);
    settle(&mut g);
    cite!(["701.41a"]; assert_eq!(plus(&g, raptor), 1));
}
