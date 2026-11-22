//! Evasion keywords lower to ordinary combat restrictions and read effective characteristics.
use crate::support::{cite, oracle};
use game::state::Decision;
use game::{CardDef, Game, Move, Target, Zone};

fn declaration(attacker: CardDef, blocker: CardDef) -> Result<(), String> {
    let mut g = Game::new(vec![attacker, blocker], 2, 20);
    g.set_ask_forced(true);
    let a = g.put(0, 0, Zone::Battlefield);
    let b = g.put(1, 1, Zone::Battlefield);
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
