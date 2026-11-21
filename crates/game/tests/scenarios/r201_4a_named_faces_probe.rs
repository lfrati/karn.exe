//! A chosen name prohibits casting only a spell with that name (601.3e, 709.3a): an
//! adventurer card cast as its Adventure has the Adventure's name (715.3b), a modal
//! double-faced card cast as its back face has that face's name (712.11b). A copy of a spell
//! with the chosen name is not cast (707.10), and a card put onto the battlefield is not cast
//! (601.1): neither is prohibited. Meddling Mage, Bonecrusher Giant, Egon.
use crate::support::{cite, paid};
use game::binding::Value;
use game::card::CastAs;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const MAGE: usize = 0;
const GIANT: usize = 1;
const EGON: usize = 2;
const BOLT: usize = 3;
const BEARS: usize = 4;
const ISLAND: usize = 5;
const FLASH_MAGE: usize = 6;
const FORK: usize = 7;
const DROP: usize = 8;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<_> = ["Meddling Mage", "Bonecrusher Giant // Stomp",
        "Egon, God of Death // Throne of Death", "Lightning Bolt", "Grizzly Bears", "Island"].map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    defs.push(CardDef::new("Flash Mage", "{0}", "Creature — Human Wizard",
        "Flash\nAs Flash Mage enters, choose a nonland card name.\nSpells with the chosen name can't be cast.", Some((2, 2))).unwrap());
    defs.push(CardDef::new("Fork", "{0}", "Instant", "Copy target spell.", None).unwrap());
    defs.push(CardDef::new("Drop", "{0}", "Sorcery", "Put a creature card from your hand onto the battlefield.", None).unwrap());
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn name(n: &str) -> Move { Move::Choose(Answer::Value(Value::CardName(n.into()))) }
fn asked_name(g: &Game, player: usize) {
    assert!(matches!(g.decision, Some(Decision::Choose { player: p, choice: Choice::Name(_) }) if p == player), "{:?}", g.decision);
}
/// Meddling Mage enters for player 0, who names `named`.
fn mage(g: &mut Game, named: &str) {
    let card = g.put(0, MAGE, Zone::Hand);
    g.players[0].pool.colors[0] += 1;
    g.players[0].pool.colors[1] += 1;
    g.act(0, paid("WU",Move::Cast { card, targets: vec![] })).unwrap();
    pass(g);
    asked_name(g, 0);
    g.act(0, name(named)).unwrap();
}
fn stomp(card: ObjId) -> Move {
    paid("RR",Move::CastAs(CastAs::Adventure, Box::new(Move::Cast { card, targets: vec![vec![Target::Player(0)]] })))
}

#[test]
fn r715_3b_naming_the_adventurer_leaves_its_adventure_castable() {
    let mut g = game();
    mage(&mut g, "Bonecrusher Giant");
    let card = g.put(0, GIANT, Zone::Hand);
    g.players[0].pool.colors[3] = 3;
    cite!(["601.3"]; refused("an effect prohibits casting that spell (601.3)"): g.act(0, paid("RRR",Move::CastAs(CastAs::Whole, Box::new(Move::Cast { card, targets: vec![] })))));
    cite!(["715.3b", "601.3e"]; ok: g.act(0, stomp(card)));
}

#[test]
fn r715_3b_naming_the_adventure_leaves_the_creature_castable() {
    let mut g = game();
    mage(&mut g, "Stomp");
    let card = g.put(0, GIANT, Zone::Hand);
    g.players[0].pool.colors[3] = 3;
    cite!(["601.3"]; refused("an effect prohibits casting that spell (601.3)"): g.act(0, stomp(card)));
    cite!(["715.3b"]; ok: g.act(0, paid("RRR",Move::Cast { card, targets: vec![] })));
}

#[test]
fn r712_11b_naming_a_back_face_prohibits_only_that_face() {
    let mut g = game();
    mage(&mut g, "Throne of Death");
    let card = g.put(0, EGON, Zone::Hand);
    g.players[0].pool.colors[2] = 3;
    cite!(["601.3"]; refused("an effect prohibits casting that spell (601.3)"): g.act(0, paid("B",Move::CastAs(CastAs::Face(1), Box::new(Move::Cast { card, targets: vec![] })))));
    cite!(["712.11b"]; ok: g.act(0, paid("BBB",Move::CastAs(CastAs::Face(0), Box::new(Move::Cast { card, targets: vec![] })))));
}

#[test]
fn r707_10_a_copy_of_a_spell_with_the_chosen_name_is_not_cast() {
    let mut g = game();
    // player 1 casts Lightning Bolt; player 0 flashes in a namer and names it
    g.act(0, Move::Pass).unwrap();
    let bolt = g.put(1, BOLT, Zone::Hand);
    g.players[1].pool.colors[3] = 1;
    g.act(1, paid("R",Move::Cast { card: bolt, targets: vec![vec![Target::Player(0)]] })).unwrap();
    let original = g.stack[0];
    g.act(1, Move::Pass).unwrap();
    let namer = g.put(0, FLASH_MAGE, Zone::Hand);
    g.act(0, Move::Cast { card: namer, targets: vec![] }).unwrap();
    pass(&mut g);
    asked_name(&g, 0);
    g.act(0, name("Lightning Bolt")).unwrap();
    assert_eq!(g.stack, vec![original], "the Bolt is still on the stack");
    let fork = g.put(0, FORK, Zone::Hand);
    g.act(0, Move::Cast { card: fork, targets: vec![vec![Target::Object(original)]] }).unwrap();
    pass(&mut g);
    while g.decision.is_some() { g.act(0, Move::Choose(Answer::No)).unwrap(); }
    cite!(["707.10"]; assert_eq!(g.stack.len(), 2, "Fork resolved and copied the named Bolt: {:?}", g.stack));
    cite!(["707.10"]; assert_eq!(g.def(*g.stack.last().unwrap()).unwrap().name(), "Lightning Bolt"));
}

#[test]
fn r601_1_a_card_with_the_chosen_name_put_onto_the_battlefield_is_not_cast() {
    let mut g = game();
    mage(&mut g, "Grizzly Bears");
    pass(&mut g);
    let turn = g.turn;
    while !(g.turn > turn && g.part == ir::Part::PrecombatMain) { g.act(g.priority, Move::Pass).unwrap(); }
    let p = g.active;
    if p != 0 { while !(g.active == 0 && g.part == ir::Part::PrecombatMain) { g.act(g.priority, Move::Pass).unwrap(); } }
    let bears = g.put(0, BEARS, Zone::Hand);
    g.players[0].pool.colors[4] = 2;
    cite!(["601.3"]; refused("an effect prohibits casting that spell (601.3)"): g.act(0, paid("GG",Move::Cast { card: bears, targets: vec![] })));
    let drop = g.put(0, DROP, Zone::Hand);
    g.act(0, Move::Cast { card: drop, targets: vec![] }).unwrap();
    pass(&mut g);
    while let Some(d) = g.decision.clone() {
        let Decision::Choose { player, .. } = d else { panic!("{d:?}") };
        g.act(player, Move::Choose(Answer::Objects(vec![bears]))).unwrap();
    }
    cite!(["601.2"]; assert!(g.battlefield.iter().any(|&o| g.def(o).unwrap().name() == "Grizzly Bears"), "Bears put onto the battlefield"));
}

