//! Sneak (702.190).
//!
//! 702.190a: "Sneak [cost]" means "Any time you could cast an instant during your declare
//! blockers step, you may cast this spell by paying [cost] and returning an unblocked creature
//! you control to its owner's hand rather than paying this spell's mana cost."
//!
//! 702.190b: "A permanent spell whose sneak cost was paid enters the battlefield tapped and
//! attacking (see rule 506.3a). It will be attacking the same player, planeswalker, or battle
//! as the creature that was returned to its owner's hand to pay the sneak cost of the spell
//! that became that permanent."
//!
//! Every sneak card is read from its Oracle text.
use crate::support::cite;
use game::announcement::CastCost;
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const NAMES: [&str; 6] = ["Wastes", "Splinter, Hamato Yoshi", "Leonardo, Leader in Blue", "Donatello's Technique", "Jennika's Technique", "Grizzly Bears"];
const WASTES: usize = 0;
const SPLINTER: usize = 1;
const LEONARDO: usize = 2;
const TECHNIQUE: usize = 3;
const JENNIKA: usize = 4;
const BEARS: usize = 5;

/// Three players, so that the defender the returned creature attacked is told apart.
fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs: Vec<_> = NAMES.map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 3, 20);
    for p in 0..3 { for _ in 0..10 { g.put(p, WASTES, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.decision_maker(g.priority), Move::Pass).unwrap(); } }
/// Player 0 attacks: to the declare attackers step, these attacks.
fn attack(g: &mut Game, attacks: Vec<(ObjId, Target)>) {
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(attacks)).unwrap();
}
/// Through the declare attackers step to the declare blockers step: these blocks.
fn block(g: &mut Game, blocks: Vec<(ObjId, ObjId)>) {
    pass(g);
    while let Some(Decision::Blockers { player }) = g.decision {
        let mine = blocks.iter().filter(|(b, _)| g.obj(*b).controller == player).cloned().collect();
        g.act(player, Move::Block(mine)).unwrap();
    }
    assert_eq!((g.part, g.priority), (Part::DeclareBlockers, 0));
}
fn black(g: &mut Game, n: u32) { g.players[0].pool.colors[2] = n; }
/// The sneak cost's row, if the legal casts offer the card for it.

fn sneak(g: &mut Game, card: ObjId, returned: ObjId, spending: &str) -> Result<(), String> {
    // each of these cards' first row is its sneak keyword
    let row = 0;
    g.act(0, crate::support::paid(spending,Move::CastFor(CastCost::Alternative(row), Box::new(Move::WithCosts(vec![vec![returned]], Box::new(Move::Cast { card, targets: vec![] })))))).map_err(|e| e.to_string())
}
fn entered(g: &Game, card: usize) -> ObjId { *g.battlefield.iter().find(|o| g.obj(**o).card == Some(card)).unwrap() }

#[test]
fn r702_190b_the_permanent_enters_tapped_and_attacking_what_the_returned_creature_attacked() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let other = g.put(0, BEARS, Zone::Battlefield);
    let splinter = g.put(0, SPLINTER, Zone::Hand);
    attack(&mut g, vec![(other, Target::Player(1)), (bears, Target::Player(2))]);
    block(&mut g, vec![]);
    black(&mut g, 1);
    sneak(&mut g, splinter, bears, "B").unwrap();
    cite!(["702.190a"]; assert_eq!(g.obj(bears).zone, Zone::Gone, "returned to its owner's hand"));
    assert_eq!(g.players[0].pool.colors[2], 0, "{{B}} paid");
    pass(&mut g);
    let now = entered(&g, SPLINTER);
    cite!(["702.190b"]; assert!(g.obj(now).tapped));
    cite!(["702.190b", "506.3a"]; assert!(g.combat.attackers.contains(&(now, Target::Player(2))), "{:?}", g.combat.attackers));
    assert!(!g.combat.attackers.iter().any(|(a, t)| *a == now && *t != Target::Player(2)));
    // it was never declared as an attacker (506.3a), and deals its combat damage: Splinter is
    // 1/3, and gives other Ninjas +1/+1, not the Bears
    pass(&mut g);
    cite!(["510.1"]; assert_eq!((g.players[1].life, g.players[2].life), (18, 19)));
}

#[test]
fn r702_190a_only_during_your_declare_blockers_step_returning_an_unblocked_creature() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let blocked = g.put(0, BEARS, Zone::Battlefield);
    let blocker = g.put(1, BEARS, Zone::Battlefield);
    let splinter = g.put(0, SPLINTER, Zone::Hand);
    let jennika = g.put(0, JENNIKA, Zone::Hand);
    black(&mut g, 1);
    // a main phase, with no attacker: not even an instant's sneak cost
    cite!(["702.190a"]; refused("a sneak cost is paid only during your declare blockers step"): unchanged(&mut g, |g| sneak(g, splinter, bears, "B")));
    g.players[0].pool.colors[3] = 1;
    cite!(["702.190a"]; refused("a sneak cost is paid only during your declare blockers step"): unchanged(&mut g, |g| sneak(g, jennika, bears, "R")));
    attack(&mut g, vec![(bears, Target::Player(1)), (blocked, Target::Player(1))]);
    black(&mut g, 1);
    g.players[0].pool.colors[3] = 1;
    cite!(["702.190a"]; refused("a sneak cost is paid only during your declare blockers step"): unchanged(&mut g, |g| sneak(g, jennika, bears, "R")));
    block(&mut g, vec![(blocker, blocked)]);
    black(&mut g, 1);
    g.players[0].pool.colors[3] = 1;
    cite!(["702.190a", "509.1h"]; refused("Grizzly Bears is not what that cost asks for or cannot pay it"): unchanged(&mut g, |g| sneak(g, splinter, blocked, "B")));
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    cite!(["702.190a"]; refused("Grizzly Bears is not what that cost asks for or cannot pay it"): unchanged(&mut g, |g| sneak(g, splinter, theirs, "B")));
    cite!(["702.190a"]; ok: sneak(&mut g.clone(), jennika, bears, "R"));
    // Full ordinary 1B is available; timing alone makes the cast illegal.
    g.players[0].pool.colorless = 1;
    cite!(["117.1a"]; refused("that is cast in a main phase of your own turn with the stack empty"): unchanged(&mut g, |g| g.act(0, crate::support::paid("CB", Move::Cast { card: splinter, targets: vec![] }))));
    sneak(&mut g, splinter, bears, "B").unwrap();
    pass(&mut g);
    // the end of combat step: the declare blockers step is over
    while g.part != Part::EndOfCombat { g.act(g.decision_maker(g.priority), Move::Pass).unwrap(); }
    g.players[0].pool.colors[3] = 1;
    let attacker = entered(&g, SPLINTER);
    cite!(["702.190a"]; refused("a sneak cost is paid only during your declare blockers step"): unchanged(&mut g, |g| sneak(g, jennika, attacker, "R")));
}

#[test]
fn r702_190a_not_on_another_players_turn() {
    let mut g = game();
    // player 0's turn passes; player 1 attacks player 0, who has a creature but no attacker
    g.put(0, BEARS, Zone::Battlefield);
    let jennika = g.put(0, JENNIKA, Zone::Hand);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    while !(g.active == 1 && matches!(g.decision, Some(Decision::Attackers { .. }))) {
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            Some(d @ Decision::Discard { .. }) => panic!("no authored cleanup discard here: {d:?}"),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    g.act(1, Move::Attack(vec![(theirs, Target::Player(0))])).unwrap();
    for _ in 0..3 { g.act(g.decision_maker(g.priority), Move::Pass).unwrap(); }
    while let Some(Decision::Blockers { player }) = g.decision { g.act(player, Move::Block(vec![])).unwrap(); }
    assert_eq!(g.part, Part::DeclareBlockers);
    while g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    g.players[0].pool.colors[3] = 1;
    cite!(["702.190a"]; refused("a sneak cost is paid only during your declare blockers step"): unchanged(&mut g, |g| sneak(g, jennika, theirs, "R")));
}

#[test]
fn r702_190a_a_sorcery_cast_for_its_sneak_cost_at_an_instants_timing() {
    // Donatello's Technique: "Sneak {U}" and "Draw two cards."
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let technique = g.put(0, TECHNIQUE, Zone::Hand);
    attack(&mut g, vec![(bears, Target::Player(1))]);
    block(&mut g, vec![]);
    let hand = g.players[0].hand.len();
    g.players[0].pool.colors[1] = 1;
    g.players[0].pool.colorless = 2;
    cite!(["702.190a"]; refused("that is cast in a main phase of your own turn with the stack empty"): unchanged(&mut g, |g| g.act(0, crate::support::paid("CCU", Move::Cast { card: technique, targets: vec![] }))));
    g.players[0].pool.colors[1] = 1;
    sneak(&mut g, technique, bears, "U").unwrap();
    pass(&mut g);
    // the Technique left, the Bears came back, two cards drawn
    cite!(["702.190a"]; assert_eq!(g.players[0].hand.len(), hand - 1 + 1 + 2));
    assert_eq!(g.obj(technique).zone, Zone::Gone);
    assert!(g.players[0].graveyard.iter().any(|c| g.obj(*c).card == Some(TECHNIQUE)));
}

#[test]
fn r702_190_if_his_sneak_cost_was_paid() {
    // Leonardo, Leader in Blue: "When Leonardo enters, if his sneak cost was paid, creatures
    // you control get +2/+0 until end of turn."
    for sneaked in [true, false] {
        let mut g = game();
        let bears = g.put(0, BEARS, Zone::Battlefield);
        let other = g.put(0, BEARS, Zone::Battlefield);
        let leonardo = g.put(0, LEONARDO, Zone::Hand);
        if sneaked {
            attack(&mut g, vec![(bears, Target::Player(1)), (other, Target::Player(1))]);
            block(&mut g, vec![]);
            g.players[0].pool.colors[0] = 5;
            sneak(&mut g, leonardo, bears, "WWWWW").unwrap();
        } else {
            g.players[0].pool.colors[0] = 1;
            g.act(0, crate::support::paid("W",Move::Cast { card: leonardo, targets: vec![] })).unwrap();
        }
        while !g.stack.is_empty() || g.battlefield.iter().all(|o| g.obj(*o).card != Some(LEONARDO)) { g.act(g.decision_maker(g.priority), Move::Pass).unwrap(); }
        while !g.stack.is_empty() { g.act(g.decision_maker(g.priority), Move::Pass).unwrap(); }
        cite!(["702.190", "603.4"]; case: sneaked; assert_eq!(g.seen(other, None).pt, Some((if sneaked { 4 } else { 2 }, 2))));
    }
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
