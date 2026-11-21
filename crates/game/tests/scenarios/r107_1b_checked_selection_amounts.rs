//! Native cards that reach checked selection amounts (107.1b). These exercise checked SetX;
//! they make no claim about other selection consumers.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone};
use game::state::{Answer, Choice, Decision};

fn native(names: &[&str]) -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs: Vec<_> = names.iter().map(|name| {
        let def = CardDef::from_faces(&catalog[*name]).unwrap();
        // Uncited (admission): compiling is a precondition, not whole-card evidence.
        game::play::check(&def).unwrap();
        def
    }).collect();
    Game::new(defs, 2, 20)
}
fn noir(charge: u32) -> (Game, game::ObjId, Vec<game::ObjId>) {
    let mut g = native(&["Spider-Man Noir", "Swamp"]);
    let source = g.put(0, 0, Zone::Battlefield);
    g.set_initial_counters(source, "charge", charge);
    let cards = (0..3).map(|_| g.put(0, 1, Zone::Library)).collect();
    while g.decision != Some(Decision::Attackers { player: 0 }) {
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(0, Move::Attack(vec![(source, Target::Player(1))])).unwrap();
    // Put the attack-alone trigger on the stack, then retain the final pass
    // as the public transaction that will attempt its resolution.
    g.act(g.priority, Move::Pass).unwrap();
    (g, source, cards)
}

// Rules: 107.1b, 608.2c, 608.2h, 701.25a
// Noir first adds one +1/+1 counter, then counts ALL counters, not only that kind.
// MAX charge + one is mathematically MAX+1; SetX already refuses before Surveil.
#[test]
fn r107_1b_selection_overflow_refuses_transactionally() {
    let (mut g, source, cards) = noir(u32::MAX);
    let before = format!("{g:?}");
    cite!(["107.1b"]; assert!(g.act(g.priority, Move::Pass).unwrap_err().contains("quantity exceeds the supported numerical range (107.1b)")));
    // Uncited (transaction contract): every rules-state field, including RNG, logs,
    // choices, priority and pending procedures, equals its pre-action state.
    assert_eq!(format!("{g:?}"), before);
    cite!(["608.2c"]; assert_eq!(g.obj(source).counters.get("+1/+1"), None));
    cite!(["608.2c"]; assert_eq!(g.obj(source).counters["charge"], u32::MAX));
    cite!(["701.25a"]; assert_eq!(g.players[0].library, cards));
    cite!(["608.2c"]; assert_eq!(g.stack.len(), 1));
    // The same pending resolution refuses again without accumulating effects.
    cite!(["107.1b"]; assert!(g.act(g.priority, Move::Pass).unwrap_err().contains("107.1b")));
    assert_eq!(format!("{g:?}"), before);
}

// Rules: 107.1b, 608.2c, 608.2h, 701.25a
// MAX-1 charge + one is exactly representable. Surveil considers all three
// available cards (609.3), freezes that private group, and waits for an answer.
#[test]
fn r107_1b_native_exact_boundary_suspends_and_preserves_the_selection() {
    let (mut g, source, cards) = noir(u32::MAX - 1);
    cite!(["107.1b"]; ok: g.act(g.priority, Move::Pass));
    cite!(["608.2c"]; assert_eq!(g.obj(source).counters["+1/+1"], 1));
    let Some(Decision::Choose { player, choice: Choice::Arrange { cards: offered, verb, .. } }) = &g.decision else { panic!("Surveil decision: {:?}", g.decision) };
    cite!(["701.25a","609.3"]; assert_eq!((*player, *verb, offered.clone()), (0, ir::Verb::Surveil, cards.iter().rev().copied().collect())));
    cite!(["701.25a"]; assert_eq!(g.players[0].library, cards));
    let before = format!("{g:?}");
    // Uncited (protocol contract): a different action cannot bypass the question.
    crate::support::refused(g.act(0, Move::Pass),"a decision is pending: a choice of the effect being applied (608.2d)");
    assert_eq!(format!("{g:?}"), before);
    for mut branch in [g.clone(), g] {
        cite!(["701.25a"]; ok: branch.act(0, Move::Choose(Answer::Arrange { top: cards.iter().rev().copied().collect(), away: vec![] })));
        cite!(["701.25a"]; assert_eq!(branch.players[0].library, cards));
        cite!(["608.2c"]; assert!(branch.stack.is_empty()));
        cite!(["701.25a"]; assert!(branch.players[0].graveyard.is_empty()));
    }
}

// Rules: 107.1b, 608.2c, 701.25a, 121.1
// With no artifacts, Cerebral Download's derived X is zero. It offers no
// library arrangement but still executes the following three draws.
#[test]
fn r107_1b_native_zero_surveil_keeps_following_draws() {
    let mut g = native(&["Cerebral Download", "Island"]);
    let spell = g.put(0, 0, Zone::Hand);
    for _ in 0..4 { g.put(0, 1, Zone::Library); }
    g.players[0].pool.colors[1] = 5;
    cite!(["601.2h"]; ok: g.act(0, paid("UUUUU", Move::Cast { card: spell, targets: vec![] })));
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["701.25a"]; assert!(g.decision.is_none()));
    cite!(["608.2c","121.1"]; assert_eq!(g.players[0].hand.len(), 3));
    cite!(["608.2c","121.1"]; assert_eq!(g.players[0].library.len(), 1));
    cite!(["608.2c"]; assert!(g.stack.is_empty()));
}

#[test]
fn native_stargaze_refuses_unrepresentable_twice_x_before_payment() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let spell = CardDef::from_faces(&catalog["Stargaze"]).unwrap();
    // Uncited (admission): native variable movement is now executable. The old
    // lowering refusal is superseded by an explicit checked quantity boundary.
    game::play::check(&spell).unwrap();
    let mut g = Game::new(vec![spell], 2, 20);
    let card = g.put(0, 0, Zone::Hand);
    // The printed XBB cost is fully payable within the represented pool. The
    // empty library is legal: look/put instructions would do as much as possible.
    g.players[0].pool.colors[2] = 2_147_483_650;
    let before = format!("{g:?}");
    // Uncited (admission/transaction contract): this is a support refusal, not
    // a Magic restriction on choosing X (107.1b permits larger integers).
    let selected = game::state::Pool { colors: [0, 0, 2_147_483_650, 0, 0], ..Default::default() };
    assert!(g.act(0, Move::Paying(selected, Box::new(Move::WithX(2_147_483_648, Box::new(Move::Cast { card, targets: vec![] })))))
        .unwrap_err().contains("quantity exceeds the supported numerical range (107.1b)"));
    assert_eq!(format!("{g:?}"), before);
}
