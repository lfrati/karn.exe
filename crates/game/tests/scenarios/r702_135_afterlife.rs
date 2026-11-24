//! Afterlife (702.135a): "Afterlife N" means "When this permanent is put into a graveyard from
//! the battlefield, create N 1/1 white and black Spirit creature tokens with flying." Each
//! instance triggers separately (702.135b). Every card is read from its Oracle text.
use crate::support::{cite, mana_pool, oracle, paid};
use game::state::Decision;
use game::{Game, Move, ObjId, Zone};

const MINISTRANT: usize = 0;
const INSURANCE: usize = 1;
const BEARS: usize = 2;

/// Ministrant of Obligation ({2}{W} 2/1, "Afterlife 2"), Afterlife Insurance and Grizzly Bears.
fn setup() -> Game {
    let defs: Vec<_> = ["Ministrant of Obligation", "Afterlife Insurance", "Grizzly Bears", "Plains"].into_iter().map(oracle).collect();
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 3, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
/// Its controller puts the abilities that triggered together on the stack in the order it
/// chooses (603.3b), and the stack resolves.
fn resolve_all(g: &mut Game) {
    for _ in 0..20 {
        if let Some(Decision::Order { player, abilities }) = g.decision.clone() {
            g.act(player, Move::Order((0..abilities.len()).collect())).unwrap();
            continue;
        }
        if g.stack.is_empty() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("the stack did not empty");
}
fn spirits(g: &Game) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name.starts_with("Spirit")).collect()
}

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Ministrant of Obligation", "Afterlife Insurance", "Imperious Oligarch", "Knight of the Last Breath"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

#[test]
fn r702_135a_when_it_dies_its_controller_creates_n_flying_spirits() {
    let mut g = setup();
    let ministrant = g.put(0, MINISTRANT, Zone::Battlefield);
    g.destroy(ministrant);
    // its leaves-the-battlefield trigger is put on the stack as a player would receive priority
    g.act(g.priority, Move::Pass).unwrap();
    cite!(["702.135a", "603.10a"]; assert_eq!(g.stack.len(), 1));
    resolve_all(&mut g);
    let made = spirits(&g);
    cite!(["702.135a"]; assert_eq!(made.len(), 2));
    for spirit in made {
        let chars = &g.obj(spirit).chars;
        cite!(["702.135a", "111.4"]; assert_eq!((g.power(spirit), g.toughness(spirit), g.obj(spirit).controller), (Some(1), Some(1), 0)));
        cite!(["702.135a"]; assert!(chars.colors.len() == 2 && g.has(spirit, "Flying"), "{chars:?}"));
    }
}

#[test]
fn r702_135a_a_given_afterlife_triggers_too() {
    // Afterlife Insurance: "Creatures you control gain afterlife 1 until end of turn. Draw a card."
    let mut g = setup();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let insurance = g.put(0, INSURANCE, Zone::Hand);
    g.players[0].pool = mana_pool("WW");
    g.act(0, paid("WW", Move::Cast { card: insurance, targets: vec![] })).unwrap();
    resolve_all(&mut g);
    g.destroy(bears);
    g.act(g.priority, Move::Pass).unwrap();
    resolve_all(&mut g);
    cite!(["702.135a", "603.10a"]; assert_eq!(spirits(&g).len(), 1));
}

#[test]
fn r702_135b_each_instance_triggers_separately() {
    // Ministrant of Obligation given afterlife 1 has afterlife 2 and afterlife 1: three Spirits
    let mut g = setup();
    let ministrant = g.put(0, MINISTRANT, Zone::Battlefield);
    let insurance = g.put(0, INSURANCE, Zone::Hand);
    g.players[0].pool = mana_pool("WW");
    g.act(0, paid("WW", Move::Cast { card: insurance, targets: vec![] })).unwrap();
    resolve_all(&mut g);
    g.destroy(ministrant);
    g.act(g.priority, Move::Pass).unwrap();
    let Some(Decision::Order { player: 0, abilities }) = g.decision.clone() else { panic!("expected trigger order: {:?}", g.decision) };
    cite!(["702.135b", "603.3b"]; assert_eq!(abilities.len(), 2));
    resolve_all(&mut g);
    cite!(["702.135a", "702.135b"]; assert_eq!(spirits(&g).len(), 3));
}
