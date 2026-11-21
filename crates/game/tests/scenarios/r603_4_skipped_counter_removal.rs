//! An intervening "if" whose instruction removes counters (603.4, 122.1): when the condition
//! no longer holds as the ability resolves, nothing is removed, the same as when it removes
//! none. Creakwood Safewright: "At the beginning of your end step, if there is an Elf card in
//! your graveyard and this creature has a -1/-1 counter on it, remove a -1/-1 counter from
//! this creature."
use crate::support::{cite, paid};
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const SAFEWRIGHT: usize = 0;
const ELF: usize = 1;
const EXILE_ELF: usize = 2;
const FOREST: usize = 3;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    let defs = vec![
        real("Creakwood Safewright"),
        c("Elf Probe", "Creature — Elf", "", Some((1, 1))),
        c("Banish Probe", "Instant", "Exile target card from a graveyard.", None),
        c("Forest", "Basic Land — Forest", "", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, FOREST, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn minus(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("-1/-1").copied().unwrap_or(0) }
/// Pass to this turn's end step, as its triggers are put on the stack.
fn end_step(g: &mut Game) {
    while g.part != Part::End {
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(d) => panic!("unexpected decision {d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}
fn resolve(g: &mut Game) { while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); } }
fn safewright(g: &mut Game) -> ObjId {
    let card = g.put(0, SAFEWRIGHT, Zone::Hand);
    g.players[0].pool.colors[2] = 1;
    g.players[0].pool.colorless = 1;
    g.act(0, paid("CB", Move::Cast { card, targets: vec![] })).unwrap();
    resolve(g);
    *g.battlefield.iter().find(|o| g.obj(**o).card == Some(SAFEWRIGHT)).unwrap()
}

#[test]
fn r603_4_the_counter_is_removed_while_an_elf_card_is_in_the_graveyard() {
    let mut g = game();
    let o = safewright(&mut g);
    assert_eq!(minus(&g, o), 3);
    g.put(0, ELF, Zone::Graveyard);
    end_step(&mut g);
    resolve(&mut g);
    cite!(["603.4", "122.1"]; assert_eq!(minus(&g, o), 2));
}

#[test]
fn r603_4_without_an_elf_card_it_does_not_trigger() {
    let mut g = game();
    let o = safewright(&mut g);
    end_step(&mut g);
    cite!(["603.4"]; assert!(g.stack.is_empty()));
    assert_eq!(minus(&g, o), 3);
}

#[test]
fn r603_4_the_elf_card_leaving_before_resolution_removes_nothing() {
    let mut g = game();
    let o = safewright(&mut g);
    let elf = g.put(0, ELF, Zone::Graveyard);
    end_step(&mut g);
    assert_eq!(g.stack.len(), 1);
    let card = g.put(0, EXILE_ELF, Zone::Hand);
    while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(elf)]] }).unwrap();
    resolve(&mut g);
    cite!(["603.4"]; assert_eq!(minus(&g, o), 3));
}
