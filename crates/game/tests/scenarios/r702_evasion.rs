//! Evasion keywords lower to ordinary combat restrictions and read effective characteristics.
use crate::support::{cite, oracle};
use game::state::Decision;
use game::{CardDef, Game, Move, Target, Zone};

fn declaration(attacker: CardDef, blocker: CardDef) -> Result<(), String> {
    declaration_with_counters(attacker, blocker, 0, 0)
}

fn declaration_with_counters(attacker: CardDef, blocker: CardDef, attack_counters: u32, block_counters: u32) -> Result<(), String> {
    let mut g = Game::new(vec![attacker, blocker], 2, 20);
    g.set_ask_forced(true);
    let a = g.put(0, 0, Zone::Battlefield);
    let b = g.put(1, 1, Zone::Battlefield);
    g.put_counters(a, "+1/+1", attack_counters);
    g.put_counters(b, "+1/+1", block_counters);
    while g.decision != Some(Decision::Attackers { player: 0 }) {
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(0, Move::Attack(vec![(a, Target::Player(1))])).unwrap();
    while g.decision != Some(Decision::Blockers { player: 1 }) {
        g.act(g.priority, Move::Pass).unwrap();
    }
    g.act(1, Move::Block(vec![(b, a)]))
}

#[test]
fn r702_36b_fear_allows_black_and_artifacts_but_refuses_other_colors() {
    let attacker = oracle("Squirming Mass");
    for blocker in ["Walking Corpse", "Memnite"] {
        cite!(["702.36b"]; ok: declaration(attacker.clone(), oracle(blocker)));
    }
    cite!(["702.36b"]; refused("Squirming Mass has fear (702.36b, 509.1b)"): declaration(attacker, oracle("Grizzly Bears")));
}

#[test]
fn r702_13b_intimidate_uses_shared_colors_and_artifact_creatures() {
    let attacker = oracle("Krenko's Enforcer");
    for blocker in ["Hill Giant", "Memnite"] {
        cite!(["702.13b"]; ok: declaration(attacker.clone(), oracle(blocker)));
    }
    cite!(["702.13b"]; refused("Krenko's Enforcer has intimidate (702.13b, 509.1b)"): declaration(attacker, oracle("Walking Corpse")));
    let colorless = CardDef::new("Colorless intimidator", "{0}", "Creature", "Intimidate", Some((2, 2))).unwrap();
    game::play::check(&colorless).unwrap();
    cite!(["702.13b", "105.2c"]; refused("Colorless intimidator has intimidate (702.13b, 509.1b)"): declaration(colorless, oracle("Kozilek's Channeler")));
}

#[test]
fn r702_28b_shadow_restricts_attackers_and_blockers_in_both_directions() {
    let shadow = oracle("Dauthi Mercenary");
    cite!(["702.28b"]; ok: declaration(shadow.clone(), shadow.clone()));
    cite!(["702.28b"]; refused("Dauthi Mercenary has shadow (702.28b, 509.1b)"): declaration(shadow.clone(), oracle("Grizzly Bears")));
    cite!(["702.28b"]; refused("Dauthi Mercenary has shadow (702.28b, 509.1b)"): declaration(oracle("Grizzly Bears"), shadow));
}

#[test]
fn r702_31b_horsemanship_only_restricts_what_can_block_its_holder() {
    let horse = oracle("Wei Scout");
    cite!(["702.31b"]; ok: declaration(horse.clone(), horse.clone()));
    cite!(["702.31b"]; refused("Wei Scout has horsemanship (702.31b, 509.1b)"): declaration(horse.clone(), oracle("Storm Crow")));
    cite!(["702.31b"]; ok: declaration(oracle("Grizzly Bears"), horse));
}

#[test]
fn r702_118b_skulk_compares_current_power_allowing_equal_and_smaller_blockers() {
    let attacker = oracle("Farbog Revenant");
    for blocker in ["Memnite", "Ornithopter"] {
        cite!(["702.118b"]; ok: declaration(attacker.clone(), oracle(blocker)));
    }
    cite!(["702.118b"]; refused("Farbog Revenant has skulk"): declaration(attacker.clone(), oracle("Grizzly Bears")));
    cite!(["702.118b", "613.4c"]; ok: declaration_with_counters(attacker.clone(), oracle("Grizzly Bears"), 1, 0));
    cite!(["702.118b", "613.4c"]; refused("Farbog Revenant has skulk"): declaration_with_counters(attacker, oracle("Grizzly Bears"), 1, 1));
}

#[test]
fn r702_118b_granted_skulk_compares_the_recipient_power_and_ends_with_its_grant() {
    let mut g = Game::new(vec![oracle("Memnite"), oracle("Grizzly Bears"), oracle("Behind the Scenes")], 2, 20);
    g.set_ask_forced(true);
    let attacker = g.put(0, 0, Zone::Battlefield);
    let blocker = g.put(1, 1, Zone::Battlefield);
    let grant = g.put(0, 2, Zone::Battlefield);
    while g.decision != Some(Decision::Attackers { player: 0 }) { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(vec![(attacker, Target::Player(1))])).unwrap();
    while g.decision != Some(Decision::Blockers { player: 1 }) { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["702.118b", "613.1f"]; refused("Memnite has skulk"): g.act(1, Move::Block(vec![(blocker, attacker)])));
    g.destroy(grant);
    cite!(["702.118b", "611.3b"]; ok: g.act(1, Move::Block(vec![(blocker, attacker)])));
}
