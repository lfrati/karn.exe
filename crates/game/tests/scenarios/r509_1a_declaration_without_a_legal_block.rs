//! The declaration of blockers is made whenever a creature attacks a player who controls a
//! creature (509.1a), even when no block would be legal. A harness replaying a record asks for
//! it (`ask_forced`), so that an illegal block can be attempted and refused (509.1b, 702.9b) and
//! the empty declaration then lets combat go on; without the flag the engine keeps skipping a
//! declaration with nothing to declare.
use crate::support::cite;
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};

fn cards() -> Vec<CardDef> {
    vec![
        CardDef::new("Skyhunter", "{0}", "Creature — Cat Knight", "Flying", Some((2, 2))).unwrap(),
        CardDef::new("Goblin", "{0}", "Creature — Goblin", "", Some((2, 2))).unwrap(),
    ]
}

/// Player 0 attacks player 1 with the flyer, player 1 controlling only a ground creature;
/// returns at player 1's declaration of blockers, or once combat damage was dealt without one.
fn attack(ask_forced: bool) -> (Game, ObjId, ObjId) {
    let mut g = Game::new(cards(), 2, 20);
    g.set_ask_forced(ask_forced);
    let flyer = g.put(0, 0, Zone::Battlefield);
    let goblin = g.put(1, 1, Zone::Battlefield);
    for _ in 0..40 {
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![(flyer, Target::Player(1))])).unwrap(),
            Some(Decision::Blockers { .. }) => return (g, flyer, goblin),
            Some(d) => panic!("{d:?}"),
            None if g.players[1].life < 20 => return (g, flyer, goblin),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("neither a declaration of blockers nor combat damage")
}

#[test]
fn asked_for_a_record_an_illegal_block_is_refused_and_the_empty_declaration_goes_on() {
    let (mut g, flyer, goblin) = attack(true);
    cite!(["509.1a"]; assert!(matches!(g.decision, Some(Decision::Blockers { player: 1 }))));
    cite!(["509.1b", "702.9b"]; assert_eq!(g.act(1, Move::Block(vec![(goblin, flyer)])),
        Err("Goblin can't block Skyhunter: Skyhunter has flying (702.9b, 509.1b)".into())));
    cite!(["509.1a"]; ok: g.act(1, Move::Block(vec![])));
    for _ in 0..20 {
        if g.players[1].life < 20 { break; }
        match g.decision.clone() {
            None => g.act(g.priority, Move::Pass).unwrap(),
            Some(d) => panic!("{d:?}"),
        }
    }
    // unblocked, the flyer deals its combat damage to the player it attacks (510.1a, 510.2)
    cite!(["510.2"]; assert_eq!(g.players[1].life, 18));
}

#[test]
fn without_the_flag_a_declaration_with_nothing_to_declare_is_not_asked() {
    let (g, _, _) = attack(false);
    cite!(["509.1a"]; assert!(g.decision.is_none()));
    assert_eq!(g.players[1].life, 18);
}
