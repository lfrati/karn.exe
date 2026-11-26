//! Poisonous (702.70a): "Poisonous N" means "Whenever this creature deals combat damage to a
//! player, that player gets N poison counters." Each instance triggers separately (702.70b).
//! Every card is read from its Oracle text.
use crate::support::{cite, oracle};
use game::state::Decision;
use game::{Game, Move, ObjId, Target, Zone};

const SLIVER: usize = 0;
const INITIATION: usize = 1;
const BEARS: usize = 2;

/// Virulent Sliver ("All Sliver creatures have poisonous 1."), Snake Cult Initiation
/// ("Enchant creature / Enchanted creature has poisonous 3.") and Grizzly Bears.
fn setup() -> Game {
    let defs: Vec<_> = ["Virulent Sliver", "Snake Cult Initiation", "Grizzly Bears"].into_iter().map(oracle).collect();
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, BEARS, Zone::Library); } }
    g
}
fn pass(g: &mut Game) { g.act(g.priority, Move::Pass).unwrap(); }
fn all_pass(g: &mut Game) { for _ in 0..g.players.len() { pass(g); } }
/// Player 0 attacks player 1 with these creatures, and player 1 makes these blocks.
fn attack(g: &mut Game, attackers: &[ObjId], blocks: Vec<(ObjId, ObjId)>) {
    all_pass(g);
    all_pass(g);
    assert_eq!(g.part, ir::Part::DeclareAttackers);
    g.act(0, Move::Attack(attackers.iter().map(|a| (*a, Target::Player(1))).collect())).unwrap();
    all_pass(g);
    if g.decision == Some(Decision::Blockers { player: 1 }) { g.act(1, Move::Block(blocks)).unwrap(); }
    else { assert!(blocks.is_empty(), "player 1 has no creature to block with"); }
}
/// Combat damage is dealt, and what it triggered resolves in the order its controller chose.
fn finish_combat(g: &mut Game) {
    for _ in 0..60 {
        if let Some(Decision::Order { player, abilities }) = g.decision.clone() {
            g.act(player, Move::Order((0..abilities.len()).collect())).unwrap();
            continue;
        }
        if g.part == ir::Part::EndOfCombat && g.stack.is_empty() { return; }
        pass(g);
    }
    panic!("combat did not finish")
}
fn poison(g: &Game, player: usize) -> u32 { g.counters(Target::Player(player)).get("poison").copied().unwrap_or(0) }

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Virulent Sliver", "Snake Cult Initiation"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

#[test]
fn r702_70a_combat_damage_to_a_player_gives_it_n_poison_counters() {
    let mut g = setup();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let aura = g.put(0, INITIATION, Zone::Battlefield);
    assert!(g.attach(aura, Target::Object(bears)));
    attack(&mut g, &[bears], vec![]);
    finish_combat(&mut g);
    cite!(["702.70a", "122.1f"]; assert_eq!(poison(&g, 1), 3));
    // the combat damage is dealt as well
    cite!(["510.2"]; assert_eq!(g.players[1].life, 18));
}

#[test]
fn r702_70a_no_poison_without_combat_damage_to_a_player() {
    let mut g = setup();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let aura = g.put(0, INITIATION, Zone::Battlefield);
    assert!(g.attach(aura, Target::Object(bears)));
    let blocker = g.put(1, BEARS, Zone::Battlefield);
    attack(&mut g, &[bears], vec![(blocker, bears)]);
    finish_combat(&mut g);
    cite!(["702.70a"]; assert_eq!(poison(&g, 1), 0));
}

#[test]
fn r702_70b_each_instance_triggers_separately() {
    // Ruling: "Poisonous 1 causes the player to get just one poison counter when a Sliver deals
    // combat damage to them, no matter how much damage that Sliver dealt." Two Virulent Slivers
    // give each Sliver poisonous 1 twice: two triggers, two counters for each.
    let mut g = setup();
    let one = g.put(0, SLIVER, Zone::Battlefield);
    let attackers = [one];
    attack(&mut g, &attackers, vec![]);
    finish_combat(&mut g);
    cite!(["702.70a"]; assert_eq!(poison(&g, 1), 1));
    let mut g = setup();
    let one = g.put(0, SLIVER, Zone::Battlefield);
    g.put(0, SLIVER, Zone::Battlefield);
    attack(&mut g, &[one], vec![]);
    finish_combat(&mut g);
    cite!(["702.70b"]; assert_eq!(poison(&g, 1), 2));
}
