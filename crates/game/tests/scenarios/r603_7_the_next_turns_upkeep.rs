//! "Draw a card at the beginning of the next turn's upkeep": a delayed triggered ability made
//! as the spell or ability resolves (603.7a). It triggers once, at the upkeep of the turn
//! after the one it was made in, whoever's turn that is (603.7b).
use crate::support;
use support::*;

// Rules: 603.7a, 603.7b
// Given: Mishra's Bauble on its controller's battlefield; every library holds cards.
// When: its controller sacrifices it in their main phase, looking at the top card of player
// 1's library; play goes on to the next turn, player 1's.
// Expect: nothing is drawn that turn; at the beginning of player 1's upkeep its controller
// draws a card, before player 1 draws for their turn.
#[test]
fn r603_7a_mishras_bauble_draws_at_the_next_turns_upkeep() {
    let mut t = Board::new(2).battlefield(0, &["Mishra's Bauble"]).fill(5).build();
    t.activate(0, "Mishra's Bauble", 0, vec![player(1)], mana_pool("")).unwrap();
    t.resolve();
    t.expect().rules(["603.7a"]).zone(0, Zone::Graveyard, &["Mishra's Bauble"]).holds(|g| g.players[0].hand.is_empty(), "nothing drawn yet").check();
    t.to_no_combat(1, Part::Upkeep);
    t.expect().rules(["603.7a", "603.7b"]).holds(|g| g.players[0].hand.len() == 1 && g.players[1].hand.is_empty(), "drawn at the next turn's upkeep").check();
}

// Rules: 603.7b
// Given: Mishra's Bauble sacrificed in its controller's main phase.
// When: play goes on to that player's own next upkeep.
// Expect: the ability triggered once, at the turn after: its controller holds that card and
// no other (they have not drawn for their turn yet).
#[test]
fn r603_7b_the_next_turns_upkeep_triggers_once() {
    let mut t = Board::new(2).battlefield(0, &["Mishra's Bauble"]).fill(5).build();
    t.activate(0, "Mishra's Bauble", 0, vec![player(1)], mana_pool("")).unwrap();
    t.resolve();
    t.to_no_combat(0, Part::Upkeep);
    cite!(["603.7b"]; assert_eq!(t.g.players[0].hand.len(), 1));
}

// Rules: 603.7a, 603.7b
// Given: Heal cast in player 1's upkeep, preventing the next 1 damage to player 1.
// When: play goes on.
// Expect: its caster draws nothing in that upkeep, where the ability was made after the step
// began, and draws at the next turn's upkeep, their own.
#[test]
fn r603_7a_heal_cast_in_an_upkeep_draws_at_the_next_turns() {
    let mut t = Board::new(2).hand(0, &["Heal"]).active(1).at(Part::Upkeep).fill(5).build();
    t.g.act(1, Move::Pass).unwrap();
    t.mana(0, "{W}").cast(0, "Heal", vec![player(1)], mana_pool("W")).unwrap();
    t.resolve();
    cite!(["603.7a"]; assert_eq!(t.g.players[0].hand.len(), 0));
    t.to_no_combat(0, Part::Upkeep);
    cite!(["603.7b"]; assert_eq!(t.g.players[0].hand.len(), 1));
}
