//! "If that creature would die this turn, exile it instead." said by a resolving spell
//! (614.1a): a replacement kept for the rest of the turn for the object it names, and that
//! object only (400.7). Beside the creature's own death replacement, its controller chooses
//! which applies (616.1).
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::Part;

const COIL: usize = 0;
const BEAR: usize = 1;
const GIANT: usize = 2;
const MURDER: usize = 3;
const WATCHER: usize = 4;
const CHOSEN: usize = 5;
const FLICKER: usize = 6;
const ISLAND: usize = 7;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    let defs = vec![
        real("Lava Coil"),
        c("Bear", "Creature — Bear", "", Some((2, 2))),
        c("Giant", "Creature — Giant", "", Some((5, 5))),
        c("Destroy probe", "Instant", "Destroy target creature.", None),
        c("Watcher", "Enchantment", "Whenever a creature dies, you gain 1 life.", None),
        real("Nissa's Chosen"),
        c("Flicker", "Instant", "Exile target creature you control, then return it to the battlefield under its owner's control.", None),
        c("Island", "Basic Land — Island", "", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    g.put(0, WATCHER, Zone::Battlefield);
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

fn cast(g: &mut Game, card: usize, target: ObjId, spending: &str) {
    let spell = g.put(0, card, Zone::Hand);
    g.players[0].pool.colors[3] = 1;
    g.players[0].pool.colorless = 1;
    g.act(0, paid(spending, Move::Cast { card: spell, targets: vec![vec![Target::Object(target)]] })).unwrap();
    while !g.stack.is_empty() && g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn exiled(g: &Game, card: usize) -> bool { g.exile.iter().any(|o| g.obj(*o).card == Some(card)) }
fn buried(g: &Game, player: usize, card: usize) -> bool { g.players[player].graveyard.iter().any(|o| g.obj(*o).card == Some(card)) }

#[test]
fn r614_1a_the_creature_is_exiled_instead_and_never_dies() {
    let mut g = game();
    let bear = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, COIL, bear, "CR");
    cite!(["614.1a"]; assert!(exiled(&g, BEAR) && !buried(&g, 1, BEAR)));
    // it did not die: nothing that triggers on dying triggered (700.4)
    cite!(["614.6"]; assert_eq!(g.players[0].life, 20));
    assert!(!g.log.iter().any(|e| matches!(e, Event::ZoneChanged { was, to: Zone::Graveyard, .. } if *was == bear)));
}

#[test]
fn r614_1a_it_lasts_the_turn_whatever_kills_it() {
    let mut g = game();
    let giant = g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, COIL, giant, "CR");
    assert!(g.battlefield.contains(&giant));
    cast(&mut g, MURDER, giant, "");
    cite!(["614.1a"]; assert!(exiled(&g, GIANT) && !buried(&g, 1, GIANT)));
    assert_eq!(g.players[0].life, 20);
}

#[test]
fn r514_2_it_ends_with_the_turn() {
    let mut g = game();
    let giant = g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, COIL, giant, "CR");
    while g.active == 0 { g.act(g.priority, Move::Pass).unwrap(); }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let murder = g.put(1, MURDER, Zone::Hand);
    g.players[1].pool.colors[3] = 1;
    g.act(1, Move::Cast { card: murder, targets: vec![vec![Target::Object(giant)]] }).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["514.2"]; assert!(buried(&g, 1, GIANT) && !exiled(&g, GIANT)));
    assert_eq!(g.players[0].life, 21);
}

#[test]
fn r400_7_a_new_object_is_not_that_creature() {
    let mut g = game();
    let giant = g.put(0, GIANT, Zone::Battlefield);
    cast(&mut g, COIL, giant, "CR");
    cast(&mut g, FLICKER, giant, "");
    let again = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(GIANT)).unwrap();
    cite!(["400.7"]; assert_ne!(again, giant));
    cast(&mut g, MURDER, again, "");
    cite!(["400.7"]; assert!(buried(&g, 0, GIANT) && !exiled(&g, GIANT)));
}

#[test]
fn r616_1_its_controller_chooses_between_two_death_replacements() {
    for (choose_exile, expected) in [(true, Zone::Exile), (false, Zone::Library)] {
        let mut g = game();
        let chosen = g.put(1, CHOSEN, Zone::Battlefield);
        cast(&mut g, COIL, chosen, "CR");
        let Some(Decision::Choose { player: 1, choice: Choice::Replacement { on: Target::Object(on), among, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
        cite!(["616.1"]; case: format!("exile {choose_exile}"); assert_eq!((on, among.len()), (chosen, 2)));
        let pick = among.into_iter().find(|id| (id.source != chosen) == choose_exile).unwrap();
        g.act(1, Move::Choose(Answer::Replacement(pick))).unwrap();
        while !g.stack.is_empty() && g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
        cite!(["616.1"]; case: format!("exile {choose_exile}"); assert_eq!(exiled(&g, CHOSEN), expected == Zone::Exile));
        cite!(["616.1"]; case: format!("exile {choose_exile}"); assert_eq!(g.players[1].library.iter().any(|o| g.obj(*o).card == Some(CHOSEN)), expected == Zone::Library));
        assert!(!buried(&g, 1, CHOSEN));
    }
}
