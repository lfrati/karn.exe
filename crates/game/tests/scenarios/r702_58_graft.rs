//! Graft (702.58a): "Graft N" means "This permanent enters with N +1/+1 counters on it" and
//! "Whenever another creature enters, if this permanent has a +1/+1 counter on it, you may move
//! a +1/+1 counter from this permanent onto that creature." To move a counter is to remove it
//! from one object and put it onto a second (122.5). Every card is read from its Oracle text.
use crate::support::{cite, mana_pool, oracle, paid};
use game::state::{Answer, Decision};
use game::{Game, Move, ObjId, Zone};

const INITIATE: usize = 0;
const BEARS: usize = 1;
const SCALES: usize = 2;

/// Simic Initiate ({G} 0/0, "Graft 1"), Grizzly Bears and Hardened Scales, with libraries to
/// draw from.
fn setup() -> Game {
    let defs: Vec<_> = ["Simic Initiate", "Grizzly Bears", "Hardened Scales", "Forest"]
        .into_iter().map(oracle).collect();
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 3, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn resolve(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
/// Player 0 casts this card for this mana and it resolves.
fn cast(g: &mut Game, card: usize, mana: &str) -> ObjId {
    let object = g.put(0, card, Zone::Hand);
    g.players[0].pool = mana_pool(mana);
    g.act(0, paid(mana, Move::Cast { card: object, targets: vec![] })).unwrap();
    resolve(g);
    *g.battlefield.iter().rev().find(|o| g.obj(**o).card == Some(card)).unwrap()
}
fn plus(g: &Game, o: ObjId) -> u32 { g.counters(o).get("+1/+1").copied().unwrap_or(0) }
/// Graft's trigger resolves and its controller answers whether to move the counter.
fn answer(g: &mut Game, yes: bool) {
    resolve(g);
    assert!(matches!(g.decision, Some(Decision::Choose { player: 0, .. })), "{:?}", g.decision);
    g.act(0, Move::Choose(if yes { Answer::Yes } else { Answer::No })).unwrap();
}

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Simic Initiate", "Llanowar Reborn", "Sporeback Troll", "Cytoplast Root-Kin"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

#[test]
fn r702_58a_it_enters_with_its_counters_and_moves_one_onto_a_creature_that_enters() {
    let mut g = setup();
    let initiate = cast(&mut g, INITIATE, "G");
    cite!(["702.58a", "614.1c"]; assert_eq!(plus(&g, initiate), 1));
    cite!(["702.58a"]; assert!(g.stack.is_empty(), "it does not trigger on its own entering"));
    let bears = cast(&mut g, BEARS, "GG");
    cite!(["702.58a", "603.2"]; assert_eq!(g.stack.len(), 1));
    answer(&mut g, true);
    cite!(["702.58a", "122.5"]; assert_eq!((plus(&g, bears), g.power(bears)), (1, Some(3))));
    // left with no counters, the 0/0 is put into its owner's graveyard
    cite!(["122.5", "704.5f"]; assert!(!g.battlefield.contains(&initiate)));
}

#[test]
fn r702_58a_the_move_is_optional_and_the_trigger_needs_a_counter() {
    let mut g = setup();
    let initiate = cast(&mut g, INITIATE, "G");
    let bears = cast(&mut g, BEARS, "GG");
    answer(&mut g, false);
    cite!(["702.58a"]; assert_eq!((plus(&g, initiate), plus(&g, bears)), (1, 0)));
    // still with its counter, it triggers again
    let second = cast(&mut g, BEARS, "GG");
    cite!(["702.58a", "603.4"]; assert_eq!(g.stack.len(), 1));
    answer(&mut g, true);
    cite!(["702.58a", "122.5"]; assert_eq!((plus(&g, initiate), plus(&g, second)), (0, 1)));
}

#[test]
fn r122_5_a_moved_counter_is_put_onto_the_other_creature_so_hardened_scales_applies() {
    // Ruling (Daghatar the Adamant): "To move a counter from one creature to another, the
    // counter is removed from the first creature and placed on the second. Any abilities that
    // care about a counter being placed on the second creature will apply."
    let mut g = setup();
    cast(&mut g, SCALES, "G");
    let initiate = cast(&mut g, INITIATE, "G");
    cite!(["702.58a", "614.1c"]; assert_eq!(plus(&g, initiate), 2));
    let bears = cast(&mut g, BEARS, "GG");
    answer(&mut g, true);
    cite!(["122.5", "614.1a"]; assert_eq!((plus(&g, initiate), plus(&g, bears)), (1, 2)));
}
