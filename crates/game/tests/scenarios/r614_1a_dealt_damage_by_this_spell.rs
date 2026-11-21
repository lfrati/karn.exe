//! "If a permanent dealt damage by Torch the Tower would die this turn, exile it instead."
//! said by a resolving spell: a replacement for the rest of the turn of whatever that spell
//! dealt damage to, asked as each would die (614.1a), and "If this spell was [kicked],
//! instead it deals 3 damage to that permanent and you scry 1.": an "instead" leading the
//! sentence is said of all of it. Torch the Tower with kicker in place of bargain.
use crate::support::{cite, paid};
use game::legal::Asked;
use game::state::Answer;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const TORCH: usize = 0;
const BEAR: usize = 1;
const GIANT: usize = 2;
const MURDER: usize = 3;
const ISLAND: usize = 4;

fn game() -> Game {
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    let defs = vec![
        c("Torch the Tower", "{R}", "Instant", "Kicker {1}\nTorch the Tower deals 2 damage to target creature or planeswalker. If this spell was kicked, instead it deals 3 damage to that permanent and you scry 1.\nIf a permanent dealt damage by Torch the Tower would die this turn, exile it instead.", None),
        c("Bear", "{0}", "Creature — Bear", "", Some((2, 2))),
        c("Giant", "{0}", "Creature — Giant", "", Some((3, 3))),
        c("Murder", "{0}", "Instant", "Destroy target creature.", None),
        c("Island", "", "Basic Land — Island", "", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

fn resolve(g: &mut Game) {
    for _ in 0..40 {
        match g.asked() {
            // scry: back on top
            Some((p, Asked::Arrange { cards, .. })) => g.act(p, Move::Choose(Answer::Arrange { top: cards, away: vec![] })).unwrap(),
            _ if g.stack.is_empty() => return,
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("the stack did not empty: {:?}", g.asked());
}
fn cast(g: &mut Game, card: usize, target: ObjId, kicked: bool, spending: &str) {
    let spell = g.put(0, card, Zone::Hand);
    g.players[0].pool.colors = [0, 0, 0, 2, 0];
    let cast = Move::Cast { card: spell, targets: vec![vec![Target::Object(target)]] };
    g.act(0, paid(spending, if kicked { Move::WithKicks(1, Box::new(cast)) } else { cast })).unwrap();
    resolve(g);
}
fn exiled(g: &Game, card: usize) -> bool { g.exile.iter().any(|o| g.obj(*o).card == Some(card)) }
fn buried(g: &Game, card: usize) -> bool { g.players[1].graveyard.iter().any(|o| g.obj(*o).card == Some(card)) }

#[test]
fn r614_1a_what_the_spell_killed_is_exiled_instead() {
    let mut g = game();
    let bear = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, TORCH, bear, false, "R");
    cite!(["614.1a"]; assert!(exiled(&g, BEAR) && !buried(&g, BEAR)));
}

#[test]
fn r614_1a_what_it_damaged_is_exiled_if_it_dies_later_this_turn_and_nothing_else_is() {
    let mut g = game();
    let giant = g.put(1, GIANT, Zone::Battlefield);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, TORCH, giant, false, "R");
    assert_eq!(g.obj(giant).damage, 2);
    cast(&mut g, MURDER, giant, false, "");
    cite!(["614.1a"]; assert!(exiled(&g, GIANT) && !buried(&g, GIANT)));
    // the Bear was not dealt damage by it
    cast(&mut g, MURDER, bear, false, "");
    cite!(["614.1a"]; assert!(buried(&g, BEAR) && !exiled(&g, BEAR)));
}

#[test]
fn r614_1a_only_this_turn() {
    let mut g = game();
    let giant = g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, TORCH, giant, false, "R");
    let turn = g.turn;
    while g.turn == turn || g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    // on the opponent's turn: their Murder of their own Giant
    let murder = g.put(1, MURDER, Zone::Hand);
    while g.priority != 1 { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(1, Move::Cast { card: murder, targets: vec![vec![Target::Object(giant)]] }).unwrap();
    resolve(&mut g);
    cite!(["611.2a"]; assert!(buried(&g, GIANT) && !exiled(&g, GIANT)));
}

#[test]
fn r608_2c_instead_leading_a_sentence_replaces_all_of_the_sentence_before_it() {
    let mut g = game();
    let giant = g.put(1, GIANT, Zone::Battlefield);
    let top = *g.players[0].library.last().unwrap();
    cast(&mut g, TORCH, giant, true, "RR");
    // 3, not 2 and then 3: and the scry was asked, of the top card
    assert!(exiled(&g, GIANT));
    cite!(["608.2c"]; assert!(g.log.iter().filter(|e| matches!(e, game::Event::DamageDealt { amount, .. } if *amount == 3)).count() == 1));
    cite!(["608.2c"]; assert!(!g.log.iter().any(|e| matches!(e, game::Event::DamageDealt { amount: 2, .. }))));
    assert_eq!(*g.players[0].library.last().unwrap(), top);
}
