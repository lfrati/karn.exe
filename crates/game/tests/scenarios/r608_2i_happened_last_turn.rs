//! "if no spells were cast last turn", "if a player cast two or more spells last turn"
//! (608.2i): the spells cast in a turn are still asked about in the turn after it, and no
//! longer in the one after that.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Zone};

const QUIET: usize = 0;
const BUSY: usize = 1;
const SIP: usize = 2;
const LAND: usize = 3;

fn game() -> Game {
    let defs = vec![
        CardDef::new("Quiet", "{1}", "Artifact", "At the beginning of each upkeep, if no spells were cast last turn, you gain 1 life.", None).unwrap(),
        CardDef::new("Busy", "{1}", "Artifact", "At the beginning of each upkeep, if a player cast two or more spells last turn, you lose 1 life.", None).unwrap(),
        CardDef::new("Sip", "{G}", "Instant", "Target player draws a card.", None).unwrap(),
        CardDef::new("Wastes", "", "Land", "", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs, 2, 20);
    for player in 0..2 { for _ in 0..12 { g.put(player, LAND, Zone::Library); } }
    g.put(0, QUIET, Zone::Battlefield);
    g.put(0, BUSY, Zone::Battlefield);
    g
}
/// This player casts a spell with mana given for it, and it resolves.
fn cast(g: &mut Game, player: usize) {
    let spell = g.put(player, SIP, Zone::Hand);
    g.players[player].pool.colors = [9, 9, 9, 9, 9];
    while g.priority != player { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(player, paid("G", Move::Cast { card: spell, targets: vec![vec![game::Target::Player(player)]] })).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    g.players[player].pool = Default::default();
}
/// Through the next turn's upkeep, whatever triggered in it resolved: the first player's
/// life after it.
fn upkeep(g: &mut Game) -> i32 {
    let turn = g.turn;
    while g.turn == turn || !g.stack.is_empty() {
        // Quiet and Busy have mutually exclusive intervening conditions: no trigger
        // ordering question is possible in these positions (603.4).
        let Some((player, game::legal::Asked::Priority)) = g.asked() else {
            panic!("expected only priority during this upkeep: {:?}", g.asked());
        };
        g.act(player, Move::Pass).unwrap();
    }
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    g.players[0].life
}

#[test]
fn a_turn_in_which_no_spell_was_cast() {
    let mut g = game();
    cite!(["608.2i", "603.4"]; assert_eq!(upkeep(&mut g), 21, "nothing was cast in the first turn"));
    cite!(["608.2i", "603.4"]; assert_eq!(upkeep(&mut g), 22, "nor in the second"));
}

#[test]
fn one_spell_is_neither_none_nor_two() {
    let mut g = game();
    cast(&mut g, 0);
    cite!(["608.2i", "603.4"]; assert_eq!(upkeep(&mut g), 20));
    // that spell was cast the turn before last
    cite!(["608.2i", "603.4"]; assert_eq!(upkeep(&mut g), 21));
}

#[test]
fn two_spells_by_one_player_and_not_one_each() {
    let mut g = game();
    cast(&mut g, 0);
    cast(&mut g, 1);
    cite!(["608.2i", "603.4"]; assert_eq!(upkeep(&mut g), 20, "two spells were cast, and no player cast two"));
    cast(&mut g, 1);
    cast(&mut g, 1);
    let mut copy = g.clone();
    cite!(["608.2i", "603.4"]; assert_eq!(upkeep(&mut g), 19, "a player cast two"));
    assert_eq!(upkeep(&mut copy), 19, "the clone preserves last-turn history");
}
