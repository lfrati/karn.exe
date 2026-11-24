//! Review of myriad (702.116a) on its rulings: the tokens enter attacking and were never
//! declared as attackers, so their own myriad doesn't trigger; and "defending player" is the
//! controller of the planeswalker the creature attacks.
use crate::support::{cite, oracle};
use game::state::{Answer, Choice, Decision};
use game::{Game, Move, Target, Zone};

// Rules: 702.116a, 508.4, 508.5
// Given: four players; Warchief Giant attacks Jace Beleren, a planeswalker player 1 controls.
// When: its myriad trigger resolves and a token is made for each opponent offered.
// Expect: defending player is player 1, so players 2 and 3 are offered; two tokens are made,
// and no myriad of theirs triggers: the stack is empty.
#[test]
fn r702_116a_the_tokens_myriad_does_not_trigger_and_a_planeswalkers_controller_is_defending() {
    // Warchief Giant ruling: "Although the tokens enter the battlefield attacking, they were
    // never declared as attackers. Abilities that trigger whenever a creature attacks won't
    // trigger, including the myriad ability of the tokens." And: "defending player" is "the
    // controller of the planeswalker the creature was attacking".
    let mut g = Game::new(vec![oracle("Warchief Giant"), oracle("Jace Beleren")], 4, 20);
    let giant = g.put(0, 0, Zone::Battlefield);
    let jace = g.put(1, 1, Zone::Battlefield);
    g.put_counters(jace, "loyalty", 3);
    while !matches!(g.decision, Some(Decision::Attackers { player: 0 })) { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Attack(vec![(giant, Target::Object(jace))])).unwrap();
    cite!(["702.116a", "603.2"]; assert_eq!(g.stack.len(), 1));
    while g.decision.is_none() && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    let Some(Decision::Choose { player: 0, choice: Choice::Holders { among, .. } }) = g.decision.clone() else {
        panic!("myriad asks which opponents to make a token for: {:?}", g.decision)
    };
    cite!(["702.116a", "508.5"]; assert_eq!(among, vec![Target::Player(2), Target::Player(3)]));
    g.act(0, Move::Choose(Answer::Holders(among))).unwrap();
    let tokens = g.battlefield.iter().filter(|o| g.obj(**o).token()).count();
    cite!(["702.116a"]; assert_eq!(tokens, 2));
    cite!(["508.4", "702.116a"]; assert!(g.stack.is_empty() && g.decision.is_none(), "the tokens' myriad did not trigger"));
}
