//! Monstrous and renowned (701.37, 702.112): monstrosity activated in response to
//! itself; a Clone of a monstrous creature isn't monstrous (701.37b: not copiable); Polis
//! Crusher's "if this creature is monstrous"; Consul's Lieutenant's "if it's renowned" and
//! renown from first-strike damage.
use crate::support::cite;
use game::state::{Answer, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const LION: usize = 0;
const DRAGON: usize = 1;
const CLONE: usize = 2;
const CRUSHER: usize = 3;
const LIEUTENANT: usize = 4;
const BEAR: usize = 5;
const AURA: usize = 6;
const ISLAND: usize = 7;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let real = |name: &str| CardDef::from_faces(&catalog[name]).unwrap();
    let defs = vec![real("Fleecemane Lion"), real("Stormbreath Dragon"), real("Clone"), real("Polis Crusher"), real("Consul's Lieutenant"),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Charm", "{0}", "Enchantment", "", None).unwrap(),
        real("Island")];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..9 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn mana(g: &mut Game) { for p in 0..2 { g.players[p].pool.colors = [20; 5]; g.players[p].pool.colorless = 20; } }
fn activate(g: &mut Game, source: ObjId, ability: usize, spending: &str) {
    mana(g);
    g.act(0, crate::support::paid(spending, Move::Activate { source, ability, targets: vec![], paying: vec![] })).unwrap();
}
fn settle(g: &mut Game) {
    while !g.stack.is_empty() || g.decision.is_some() {
        match g.decision.clone() {
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}
fn plus(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("+1/+1").copied().unwrap_or(0) }
/// Play to player 0's declare attackers on their next turn, attack player 1 with `attackers`,
/// and play out combat; `targets` answers any target choice.
fn attack(g: &mut Game, attackers: &[ObjId], targets: Option<Vec<Vec<Target>>>) {
    let turn = g.turn;
    while g.turn == turn || g.decision != Some(Decision::Attackers { player: 0 }) {
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    g.act(0, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap();
    while g.part != ir::Part::EndOfCombat {
        match g.decision.clone() {
            Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            Some(Decision::Targets { player, .. }) => g.act(player, Move::ChooseTargets(targets.clone().expect("targets"))).unwrap(),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}

/// Two Stormbreath monstrosities on the stack: the first makes it monstrous, the second does
/// nothing, and "when this creature becomes monstrous" triggers once.
#[test]
fn monstrosity_in_response_to_itself_happens_once() {
    let mut g = game();
    let dragon = g.put(0, DRAGON, Zone::Battlefield);
    for _ in 0..2 { g.put(1, ISLAND, Zone::Hand); }
    let hand = g.players[1].hand.len() as i32;
    activate(&mut g, dragon, 1, "CCCCCRR");
    activate(&mut g, dragon, 1, "CCCCCRR");
    assert_eq!(g.stack.len(), 2);
    settle(&mut g);
    cite!(["701.37a"]; assert_eq!(plus(&g, dragon), 3));
    assert!(g.obj(dragon).monstrous);
    cite!(["701.37a", "603.2"]; assert_eq!(g.players[1].life, 20 - hand, "one trigger"));
}

/// A Clone copying a monstrous Fleecemane Lion is a Fleecemane Lion that isn't monstrous: no
/// hexproof, no indestructible; its own monstrosity then works.
#[test]
fn a_clone_of_a_monstrous_creature_isnt_monstrous() {
    let mut g = game();
    let lion = g.put(0, LION, Zone::Battlefield);
    activate(&mut g, lion, 0, "CCCGW");
    settle(&mut g);
    assert!(g.obj(lion).monstrous);
    let clone = g.put(0, CLONE, Zone::Hand);
    mana(&mut g);
    g.act(0, crate::support::paid("CCCU", Move::Cast { card: clone, targets: vec![] })).unwrap();
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Choose(Answer::Objects(vec![lion]))).unwrap();
    settle(&mut g);
    let copy = *g.battlefield.iter().find(|o| **o != lion && g.obj(**o).chars.name == "Fleecemane Lion").expect("a copy");
    cite!(["701.37b", "707.2"]; assert!(!g.obj(copy).monstrous));
    cite!(["701.37b"]; assert!(!g.has(copy, "Hexproof") && !g.has(copy, "Indestructible")));
    cite!(["707.2"]; assert_eq!(plus(&g, copy), 0, "counters aren't copied either"));
    activate(&mut g, copy, 0, "CCCGW");
    settle(&mut g);
    cite!(["701.37a"]; assert!(g.obj(copy).monstrous && g.has(copy, "Indestructible")));
}

/// Polis Crusher: "Whenever this creature deals combat damage to a player, if this creature is
/// monstrous, destroy target enchantment that player controls." Only once it is monstrous.
#[test]
fn polis_crusher_destroys_an_enchantment_only_when_monstrous() {
    for monstrous in [false, true] {
        let mut g = game();
        let crusher = g.put(0, CRUSHER, Zone::Battlefield);
        let charm = g.put(1, AURA, Zone::Battlefield);
        if monstrous { activate(&mut g, crusher, 1, "CCCCRG"); settle(&mut g); assert!(g.obj(crusher).monstrous); }
        attack(&mut g, &[crusher], Some(vec![vec![Target::Object(charm)]]));
        cite!(["701.37a"]; case: format!("monstrous={monstrous}"); assert_eq!(g.players[1].life, 20 - if monstrous { 7 } else { 4 }));
        cite!(["603.4"]; case: format!("monstrous={monstrous}"); assert_eq!(g.battlefield.contains(&charm), !monstrous));
    }
}

/// Consul's Lieutenant (first strike, renown 1): its first-strike damage makes it renowned
/// before regular damage; on its next attack, "if it's renowned", other attackers get +1/+1.
#[test]
fn consuls_lieutenant_renowned_pumps_the_others_next_attack() {
    let mut g = game();
    let lieutenant = g.put(0, LIEUTENANT, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    attack(&mut g, &[lieutenant, bear], None);
    cite!(["702.112a"]; assert!(g.obj(lieutenant).renowned));
    cite!(["702.112a"]; assert_eq!(plus(&g, lieutenant), 1));
    cite!(["603.4"]; assert_eq!(g.players[1].life, 20 - 2 - 2, "not renowned as it attacked: the Bear deals 2"));
    attack(&mut g, &[lieutenant, bear], None);
    cite!(["603.4", "702.112b"]; assert_eq!(g.players[1].life, 16 - 3 - 3, "renowned: the Bear gets +1/+1, the Lieutenant has its counter"));
    cite!(["702.112a"]; assert_eq!(plus(&g, lieutenant), 1, "renown does nothing again"));
}
